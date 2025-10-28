use binrw::{BinRead, NullString};
use eyre_pretty::Result;
use petgraph::{Graph, graph::NodeIndex};
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub struct VirtualFile {
    pub name: String,
    pub data_offset: u32,
    pub data_length: u32,
}

#[derive(Debug)]
pub struct VirtualDir {
    pub name: String,
}

#[derive(Debug)]
pub enum VirtualEntry {
    File(VirtualFile),
    Dir(VirtualDir),
}

pub type VfsEntryId = NodeIndex;
pub type VfsGraph = Graph<VirtualEntry, ()>;

/// A virtual representation of the FileSystem in a .iso.
#[derive(Debug)]
pub struct VirtualFileSystem {
    root: VfsEntryId,
    graph: VfsGraph,
}

impl VirtualFileSystem {
    pub fn new(iso: &mut iso::Iso<impl Read + Seek>) -> Result<Self> {
        let filesystem = iso.filesystem()?;
        let mut reader = iso.reader();
        let mut graph = Graph::new();
        let root = graph.add_node(VirtualEntry::Dir(VirtualDir { name: "".into() }));

        let mut dir_stack = vec![root];
        let mut end_stack = vec![filesystem.root.entry_count];
        for (index, entry) in filesystem.entries.iter().enumerate() {
            while index as u32 + 1 == *end_stack.last().unwrap() {
                dir_stack.pop();
                end_stack.pop();
            }

            match entry {
                iso::filesystem::Entry::File(file) => {
                    reader.seek(SeekFrom::Start(
                        (filesystem.strings_offset + file.name_offset) as u64,
                    ))?;
                    let name = NullString::read(&mut reader)?.to_string();
                    let node = graph.add_node(VirtualEntry::File(VirtualFile {
                        name,
                        data_offset: file.data_offset,
                        data_length: file.data_length,
                    }));

                    graph.add_edge(*dir_stack.last().unwrap(), node, ());
                }
                iso::filesystem::Entry::Directory(dir) => {
                    reader.seek(SeekFrom::Start(
                        (filesystem.strings_offset + dir.name_offset) as u64,
                    ))?;
                    let name = NullString::read(&mut reader)?.to_string();
                    let node = graph.add_node(VirtualEntry::Dir(VirtualDir { name }));
                    graph.add_edge(*dir_stack.last().unwrap(), node, ());

                    dir_stack.push(node);
                    end_stack.push(dir.end_index);
                }
            }
        }

        Ok(VirtualFileSystem { root, graph })
    }

    pub fn root(&self) -> NodeIndex {
        self.root
    }

    pub fn graph(&self) -> &Graph<VirtualEntry, ()> {
        &self.graph
    }

    pub fn path_to_entry(&self, path: impl AsRef<str>) -> Option<VfsEntryId> {
        let mut segments = path.as_ref().rsplit("/").collect::<Vec<_>>();
        let mut current = self.root;
        'outer: loop {
            for id in self.graph.neighbors(current) {
                let child = self.graph.node_weight(id).unwrap();
                match child {
                    VirtualEntry::File(file) => {
                        if file.name == *segments.last().unwrap() {
                            if segments.len() == 1 {
                                return Some(id);
                            }

                            return None;
                        }
                    }
                    VirtualEntry::Dir(dir) => {
                        if dir.name == *segments.last().unwrap() {
                            if segments.len() == 1 {
                                return Some(id);
                            }

                            segments.pop();
                            current = id;
                            continue 'outer;
                        }
                    }
                }
            }

            return None;
        }
    }
}
