use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum MetalinkError {
    #[error("XML parsing error: {0}")]
    Parse(#[from] quick_xml::Error),
    #[error("Invalid metalink format")]
    InvalidFormat,
}

pub struct Metalink {
    pub files: Vec<MetalinkFile>,
}

pub struct MetalinkFile {
    pub name: String,
    pub size: u64,
    pub hashes: HashMap<String, String>,
    pub urls: Vec<MetalinkUrl>,
}

pub struct MetalinkUrl {
    pub url: String,
    pub priority: u8,
    pub location: Option<String>,
}

pub fn parse_metalink(xml: &str) -> Result<Metalink, MetalinkError> {
    let mut reader = Reader::from_str(xml);
    
    let mut metalink = Metalink { files: Vec::new() };
    let mut current_file = None;
    let mut current_url = None;
    let mut current_hash_type = None;
    let mut in_hashes = false;
    let mut in_size = false;
    
    let mut buf = Vec::new();
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"file" => {
                    let mut name = String::new();
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"name" {
                            if let Ok(n) = std::str::from_utf8(&a.value) {
                                name = n.to_string();
                            }
                        }
                    }
                    current_file = Some(MetalinkFile {
                        name,
                        size: 0,
                        hashes: HashMap::new(),
                        urls: Vec::new(),
                    });
                }
                b"url" => {
                    let mut priority = 99;
                    let mut location = None;
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"priority" {
                            if let Ok(p) = std::str::from_utf8(&a.value) {
                                priority = p.parse().unwrap_or(99);
                            }
                        } else if a.key.as_ref() == b"location" {
                            if let Ok(l) = std::str::from_utf8(&a.value) {
                                location = Some(l.to_string());
                            }
                        }
                    }
                    current_url = Some(MetalinkUrl {
                        url: String::new(),
                        priority,
                        location,
                    });
                }
                b"hash" => {
                    in_hashes = true;
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"type" {
                            if let Ok(t) = std::str::from_utf8(&a.value) {
                                current_hash_type = Some(t.to_string());
                            }
                        }
                    }
                }
                b"size" => {
                    in_size = true;
                }
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().ok().map(|s| s.trim().to_string());
                
                if let Some(text) = text {
                    if text.is_empty() {
                        buf.clear();
                        continue;
                    }
                    if in_hashes {
                        if let (Some(ref mut file), Some(hash_type)) = (&mut current_file, &current_hash_type) {
                            file.hashes.insert(hash_type.clone(), text);
                        }
                    } else if let Some(ref mut url) = current_url {
                        url.url = text;
                    } else if in_size {
                        if let Some(ref mut file) = current_file {
                            file.size = text.parse().unwrap_or(0);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"file" => {
                    if let Some(file) = current_file.take() {
                        metalink.files.push(file);
                    }
                }
                b"url" => {
                    if let Some(url) = current_url.take() {
                        if let Some(ref mut file) = current_file {
                            file.urls.push(url);
                        }
                    }
                }
                b"hash" => {
                    in_hashes = false;
                    current_hash_type = None;
                }
                b"size" => {
                    in_size = false;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(MetalinkError::Parse(e)),
            _ => {}
        }
        buf.clear();
    }
    
    Ok(metalink)
}
