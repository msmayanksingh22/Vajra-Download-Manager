using System.Diagnostics;
using System.IO;
using System.Net.Http;
using Vajra.Windows.Models;
using Vajra.Windows.Services;
using Xunit;

namespace Vajra.Windows.Tests;

public class EndToEndDaemonIntegrationTests
{
    private static string FindVajradBinary()
    {
        // Check relative from test execution directory
        string[] candidates = {
            Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../target/debug/vajrad.exe")),
            Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../target/release/vajrad.exe")),
            Path.GetFullPath("D:/Project/Project-Vajra/target/debug/vajrad.exe")
        };

        foreach (var c in candidates)
        {
            if (File.Exists(c)) return c;
        }

        throw new FileNotFoundException("vajrad.exe was not found in target directories.");
    }

    [Fact]
    public async Task CompleteDaemonLifecycleAndDownloadFlow_Succeeds()
    {
        string vajradPath = FindVajradBinary();
        string tempDir = Path.Combine(Path.GetTempPath(), "Vajra_E2E_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        ushort testPort = 6299;

        var startInfo = new ProcessStartInfo
        {
            FileName = vajradPath,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        startInfo.EnvironmentVariables["VAJRA_DATA_DIR"] = tempDir;
        startInfo.EnvironmentVariables["VAJRA_PORT"] = testPort.ToString();

        using var daemonProcess = Process.Start(startInfo);
        Assert.NotNull(daemonProcess);

        var stdoutSb = new System.Text.StringBuilder();
        var stderrSb = new System.Text.StringBuilder();
        daemonProcess.OutputDataReceived += (_, e) => { if (e.Data != null) lock (stdoutSb) stdoutSb.AppendLine(e.Data); };
        daemonProcess.ErrorDataReceived += (_, e) => { if (e.Data != null) lock (stderrSb) stderrSb.AppendLine(e.Data); };
        daemonProcess.BeginOutputReadLine();
        daemonProcess.BeginErrorReadLine();

        try
        {
            // 1. Wait for daemon to bootstrap and respond to health check
            string tokenFile = Path.Combine(tempDir, "api.token");
            var healthClient = new HttpClient { Timeout = TimeSpan.FromSeconds(2) };
            bool isHealthy = false;

            for (int i = 0; i < 20; i++)
            {
                await Task.Delay(300);
                if (daemonProcess.HasExited) break;

                try
                {
                    var resp = await healthClient.GetAsync($"http://127.0.0.1:{testPort}/health");
                    if (resp.IsSuccessStatusCode)
                    {
                        isHealthy = true;
                        break;
                    }
                }
                catch
                {
                    // keep waiting
                }
            }

            Assert.True(isHealthy, $"Daemon failed to start. Exited: {daemonProcess.HasExited}, ExitCode: {(daemonProcess.HasExited ? daemonProcess.ExitCode : -1)}, Stderr: {stderrSb}, Stdout: {stdoutSb}");
            Assert.True(File.Exists(tokenFile), $"Daemon should have bootstrapped api.token file at {tokenFile}.");

            string generatedToken = File.ReadAllText(tokenFile).Trim();
            Assert.True(TokenService.IsValidToken(generatedToken), "Generated token must be valid.");

            // 2. Test TokenService resolution
            var tokenService = new MockTokenService
            {
                Token = generatedToken,
                BaseUrl = $"http://127.0.0.1:{testPort}",
                Port = testPort
            };

            var daemonClient = new DaemonClient(tokenService);

            // 3. Test CheckHealthAsync
            var health = await daemonClient.CheckHealthAsync();
            Assert.True(health != null, $"CheckHealthAsync failed. LastError: {daemonClient.LastError}");
            Assert.Equal("ok", health.Status);

            // 4. Test GetDownloadsAsync on clean daemon
            var initialList = await daemonClient.GetDownloadsAsync();
            Assert.NotNull(initialList);
            Assert.Empty(initialList);

            // 5. Test SSE Connection
            var sseService = new SseService(tokenService);
            var eventReceivedTcs = new TaskCompletionSource<bool>();
            sseService.OnAdded += (added) => eventReceivedTcs.TrySetResult(true);
            sseService.OnProgress += (p) => eventReceivedTcs.TrySetResult(true);
            sseService.OnStateChange += (s) => eventReceivedTcs.TrySetResult(true);

            using var sseCts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            await sseService.StartAsync(sseCts.Token);

            // Wait a brief moment for SSE stream to establish
            await Task.Delay(500);

            // Start isolated mock HTTP file server on port 6301
            using var mockFileServer = new System.Net.HttpListener();
            mockFileServer.Prefixes.Add("http://127.0.0.1:6301/");
            mockFileServer.Start();
            using var serverCts = new CancellationTokenSource();
            _ = Task.Run(async () =>
            {
                while (!serverCts.IsCancellationRequested && mockFileServer.IsListening)
                {
                    try
                    {
                        var ctx = await mockFileServer.GetContextAsync();
                        _ = Task.Run(async () =>
                        {
                            try
                            {
                                byte[] body = System.Text.Encoding.UTF8.GetBytes("hello from mock vajra download test file 12345");
                                ctx.Response.StatusCode = 200;
                                ctx.Response.ContentType = "text/plain";
                                ctx.Response.Headers["Accept-Ranges"] = "bytes";
                                ctx.Response.ContentLength64 = body.Length;
                                if (ctx.Request.HttpMethod != "HEAD")
                                {
                                    await ctx.Response.OutputStream.WriteAsync(body, 0, body.Length);
                                }
                                ctx.Response.Close();
                            }
                            catch
                            {
                                // ignore client disconnects
                            }
                        });
                    }
                    catch
                    {
                        break;
                    }
                }
            });

            // 6. Test AddDownloadAsync
            string downloadUrl = "http://127.0.0.1:6301/testfile.txt";
            var addResult = await daemonClient.AddDownloadAsync(downloadUrl, "testfile.txt");
            Assert.NotNull(addResult);
            Assert.False(string.IsNullOrEmpty(addResult.Id));

            string downloadId = addResult.Id;

            // 7. Verify SSE event triggered
            var completedTask = await Task.WhenAny(eventReceivedTcs.Task, Task.Delay(3000));
            Assert.True(completedTask == eventReceivedTcs.Task, "SSE event should have been received after adding download.");

            // 8. Verify download list includes newly added download
            var listAfterAdd = await daemonClient.GetDownloadsAsync();
            Assert.True(listAfterAdd.Any(d => d.Id.Equals(downloadId, StringComparison.OrdinalIgnoreCase)), $"Expected to find download {downloadId} in list. ListCount: {listAfterAdd.Count}, LastError: {daemonClient.LastError}");

            // 9. Test PauseDownloadAsync
            bool pauseOk = await daemonClient.PauseDownloadAsync(downloadId);
            Assert.True(pauseOk, "PauseDownloadAsync should succeed.");

            // 10. Test ResumeDownloadAsync
            bool resumeOk = await daemonClient.ResumeDownloadAsync(downloadId);
            Assert.True(resumeOk, "ResumeDownloadAsync should succeed.");

            // 11. Test DeleteDownloadAsync
            bool deleteOk = await daemonClient.DeleteDownloadAsync(downloadId, deleteFile: false);
            Assert.True(deleteOk, "DeleteDownloadAsync should succeed.");

            var listAfterDelete = await daemonClient.GetDownloadsAsync();
            Assert.DoesNotContain(listAfterDelete, d => d.Id.Equals(downloadId, StringComparison.OrdinalIgnoreCase));

            // 12. Test SSE Disconnect & Clean Stop
            await sseService.StopAsync();
        }
        catch (Exception ex)
        {
            throw new Exception($"TEST FAILED: {ex.Message}\nDAEMON STDERR:\n{stderrSb}\nDAEMON STDOUT:\n{stdoutSb}", ex);
        }
        finally
        {
            try
            {
                if (!daemonProcess.HasExited)
                {
                    daemonProcess.Kill();
                    daemonProcess.WaitForExit(2000);
                }
            }
            catch
            {
                // ignore
            }

            try
            {
                if (Directory.Exists(tempDir))
                {
                    Directory.Delete(tempDir, recursive: true);
                }
            }
            catch
            {
                // transient file locks in temp directory
            }
        }
    }
}
