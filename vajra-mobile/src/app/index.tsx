import React, { useEffect, useState } from 'react';
import { View, Text, FlatList, StyleSheet } from 'react-native';

interface Download {
  id: string;
  filename: string;
  status: string;
  bytes_done: number;
}

export default function Index() {
  const [downloads, setDownloads] = useState<Download[]>([]);
  
  // Replace with actual daemon IP on the local network
  const DAEMON_URL = 'http://127.0.0.1:6277';
  const AUTH_TOKEN = 'YOUR_SECRET_TOKEN';

  useEffect(() => {
    fetch(`${DAEMON_URL}/api/v1/downloads`, {
      headers: { Authorization: `Bearer ${AUTH_TOKEN}` }
    })
      .then(res => res.json())
      .then(data => setDownloads(data))
      .catch(err => console.error("Failed to fetch downloads", err));
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.header}>Vajra Downloads</Text>
      <FlatList
        data={downloads}
        keyExtractor={(item) => item.id || Math.random().toString()}
        renderItem={({ item }) => (
          <View style={styles.item}>
            <Text style={styles.title} numberOfLines={1}>{item.filename}</Text>
            <Text>{item.status} - {Math.round(item.bytes_done / 1024 / 1024)} MB</Text>
          </View>
        )}
        ListEmptyComponent={<Text>No active downloads.</Text>}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, padding: 20, backgroundColor: '#fff', paddingTop: 50 },
  header: { fontSize: 24, fontWeight: 'bold', marginBottom: 20 },
  item: { padding: 15, borderBottomWidth: 1, borderColor: '#eee' },
  title: { fontSize: 16, fontWeight: '500' },
});
