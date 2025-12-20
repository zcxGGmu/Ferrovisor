//! RISC-V Flattened Device Tree (FDT) Support
//!
//! This module provides FDT parsing and manipulation functionality including:
//! - FDT structure parsing
//! - Node and property access
//! - FDT modification
//! - Device tree memory management

use crate::arch::riscv64::*;
use core::slice;
use core::str;

/// FDT header structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FdtHeader {
    /// Magic number (0xd00dfeed)
    pub magic: u32,
    /// Total size of FDT
    pub totalsize: u32,
    /// Size of structure block
    pub off_dt_struct: u32,
    /// Size of strings block
    pub off_dt_strings: u32,
    /// Size of memory reserve map
    pub off_mem_rsvmap: u32,
    /// Version of FDT
    pub version: u32,
    /// Last compatible version
    pub last_comp_version: u32,
    /// Boot CPU ID
    pub boot_cpuid_phys: u32,
    /// Size of strings block (new)
    pub size_dt_strings: u32,
    /// Size of structure block (new)
    pub size_dt_struct: u32,
}

impl FdtHeader {
    /// Check if header is valid
    pub fn is_valid(&self) -> bool {
        self.magic == 0xd00dfeed &&
        self.totalsize >= core::mem::size_of::<FdtHeader>() as u32 &&
        self.version >= 16 &&
        (self.off_dt_struct + self.size_dt_struct) <= self.totalsize &&
        (self.off_dt_strings + self.size_dt_strings) <= self.totalsize
    }

    /// Get structure block offset
    pub fn get_struct_block_offset(&self) -> usize {
        self.off_dt_struct as usize
    }

    /// Get strings block offset
    pub fn get_strings_block_offset(&self) -> usize {
        self.off_dt_strings as usize
    }

    /// Get memory reserve map offset
    pub fn get_mem_reserve_map_offset(&self) -> usize {
        self.off_mem_rsvmap as usize
    }
}

/// FDT token types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FdtToken {
    BeginNode = 0x1,
    EndNode = 0x2,
    Prop = 0x3,
    Nop = 0x4,
    End = 0x9,
}

impl FdtToken {
    /// Try to convert from u32
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0x1 => Some(FdtToken::BeginNode),
            0x2 => Some(FdtToken::EndNode),
            0x3 => Some(FdtToken::Prop),
            0x4 => Some(FdtToken::Nop),
            0x9 => Some(FdtToken::End),
            _ => None,
        }
    }
}

/// Memory reserve map entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemReserveEntry {
    /// Starting address
    pub address: u64,
    /// Size of reserved region
    pub size: u64,
}

impl MemReserveEntry {
    /// Create a new reserve entry
    pub const fn new(address: u64, size: u64) -> Self {
        Self { address, size }
    }

    /// Check if this is the end marker
    pub fn is_end(&self) -> bool {
        self.address == 0 && self.size == 0
    }
}

/// Property data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyType {
    Empty,
    U32,
    U64,
    String,
    PropEncodedArray,
    ByteArray,
    Reg,
    Phandle,
    PhandleArray,
}

/// Device tree property
#[derive(Debug, Clone)]
pub struct Property {
    /// Property name
    pub name: String,
    /// Property data
    pub data: Vec<u8>,
    /// Property type
    pub prop_type: PropertyType,
}

impl Property {
    /// Create a new property
    pub fn new(name: &str, data: Vec<u8>) -> Self {
        let prop_type = Self::infer_type(&data);

        Self {
            name: name.to_string(),
            data,
            prop_type,
        }
    }

    /// Infer property type from data
    fn infer_type(data: &[u8]) -> PropertyType {
        if data.is_empty() {
            PropertyType::Empty
        } else if data.len() == 4 {
            PropertyType::U32
        } else if data.len() == 8 {
            PropertyType::U64
        } else if data.iter().any(|&b| b == 0) {
            // Contains null terminator, likely a string
            if data[data.len() - 1] == 0 {
                PropertyType::String
            } else {
                PropertyType::ByteArray
            }
        } else {
            // Try to decode as string
            if let Ok(_) = str::from_utf8(data) {
                PropertyType::String
            } else {
                PropertyType::ByteArray
            }
        }
    }

    /// Get property as u32
    pub fn as_u32(&self) -> Option<u32> {
        if self.data.len() == 4 {
            Some(u32::from_be_bytes([
                self.data[0],
                self.data[1],
                self.data[2],
                self.data[3],
            ]))
        } else {
            None
        }
    }

    /// Get property as u64
    pub fn as_u64(&self) -> Option<u64> {
        if self.data.len() == 8 {
            Some(u64::from_be_bytes([
                self.data[0],
                self.data[1],
                self.data[2],
                self.data[3],
                self.data[4],
                self.data[5],
                self.data[6],
                self.data[7],
            ]))
        } else {
            None
        }
    }

    /// Get property as string
    pub fn as_string(&self) -> Option<&str> {
        if self.prop_type == PropertyType::String {
            str::from_utf8(&self.data).ok()
        } else {
            None
        }
    }

    /// Get property as byte array
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get property length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if property is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Device tree node
#[derive(Debug, Clone)]
pub struct Node {
    /// Node name
    pub name: String,
    /// Node properties
    pub properties: Vec<Property>,
    /// Child nodes
    pub children: Vec<Node>,
    /// Parent node (None for root)
    pub parent: Option<usize>,
    /// Node depth
    pub depth: u32,
}

impl Node {
    /// Create a new node
    pub fn new(name: &str, depth: u32) -> Self {
        Self {
            name: name.to_string(),
            properties: Vec::new(),
            children: Vec::new(),
            parent: None,
            depth,
        }
    }

    /// Add a property
    pub fn add_property(&mut self, prop: Property) {
        self.properties.push(prop);
    }

    /// Add a child node
    pub fn add_child(&mut self, child: Node) -> usize {
        let index = self.children.len();
        self.children.push(child);
        index
    }

    /// Get property by name
    pub fn get_property(&self, name: &str) -> Option<&Property> {
        self.properties.iter().find(|p| p.name == name)
    }

    /// Get property as u32
    pub fn get_prop_u32(&self, name: &str) -> Option<u32> {
        self.get_property(name)?.as_u32()
    }

    /// Get property as u64
    pub fn get_prop_u64(&self, name: &str) -> Option<u64> {
        self.get_property(name)?.as_u64()
    }

    /// Get property as string
    pub fn get_prop_string(&self, name: &str) -> Option<&str> {
        self.get_property(name)?.as_string()
    }

    /// Get property as byte array
    pub fn get_prop_bytes(&self, name: &str) -> Option<&[u8]> {
        self.get_property(name).map(|p| p.as_bytes())
    }

    /// Find child node by name
    pub fn find_child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.name == name)
    }

    /// Find descendant node by path
    pub fn find_path(&self, path: &str) -> Option<&Node> {
        if path.is_empty() {
            return Some(self);
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Some(self);
        }

        let mut current = self;
        for part in parts {
            match current.find_child(part) {
                Some(node) => current = node,
                None => return None,
            }
        }

        Some(current)
    }

    /// Get full path of this node
    pub fn get_full_path(&self) -> String {
        if self.depth == 0 {
            return "/".to_string();
        }

        // This would need parent reference to build full path
        self.name.clone()
    }
}

/// Flattened Device Tree
#[derive(Debug)]
pub struct FlattenedDeviceTree {
    /// Raw FDT data
    pub data: Vec<u8>,
    /// Header
    pub header: FdtHeader,
    /// Root node
    pub root: Option<Node>,
    /// Memory reserve entries
    pub mem_reserve: Vec<MemReserveEntry>,
}

impl FlattenedDeviceTree {
    /// Create FDT from raw data
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, &'static str> {
        if data.len() < core::mem::size_of::<FdtHeader>() {
            return Err("Invalid FDT: too small");
        }

        let header = unsafe {
            *(data.as_ptr() as *const FdtHeader)
        };

        if !header.is_valid() {
            return Err("Invalid FDT header");
        }

        let mut fdt = Self {
            data,
            header,
            root: None,
            mem_reserve: Vec::new(),
        };

        // Parse memory reserve map
        fdt.parse_mem_reserve_map()?;

        // Parse device tree structure
        fdt.parse_structure()?;

        Ok(fdt)
    }

    /// Parse memory reserve map
    fn parse_mem_reserve_map(&mut self) -> Result<(), &'static str> {
        let offset = self.header.get_mem_reserve_map_offset();
        let mut current_offset = offset;

        loop {
            if current_offset + core::mem::size_of::<MemReserveEntry>() > self.data.len() {
                return Err("Invalid memory reserve map");
            }

            let entry = unsafe {
                *(self.data.as_ptr().add(current_offset) as *const MemReserveEntry)
            };

            if entry.is_end() {
                break;
            }

            self.mem_reserve.push(entry);
            current_offset += core::mem::size_of::<MemReserveEntry>();
        }

        Ok(())
    }

    /// Parse device tree structure
    fn parse_structure(&mut self) -> Result<(), &'static str> {
        let struct_offset = self.header.get_struct_block_offset();
        let strings_offset = self.header.get_strings_block_offset();
        let struct_end = struct_offset + self.header.size_dt_struct as usize;

        let mut current_offset = struct_offset;
        let mut node_stack: Vec<Node> = Vec::new();
        let mut parent_stack: Vec<usize> = Vec::new();

        // Create root node
        let root = Node::new("", 0);
        self.root = Some(root);
        node_stack.push(self.root.as_mut().unwrap());

        while current_offset < struct_end {
            // Read token
            let token = u32::from_be_bytes([
                self.data[current_offset],
                self.data[current_offset + 1],
                self.data[current_offset + 2],
                self.data[current_offset + 3],
            ]);

            current_offset += 4;

            match FdtToken::from_u32(token) {
                Some(FdtToken::BeginNode) => {
                    // Read node name
                    let (name, name_len) = self.read_string_at(current_offset)?;
                    current_offset = align_up(current_offset + name_len + 1, 4);

                    let depth = parent_stack.len() as u32;
                    let mut node = Node::new(name, depth);

                    if let Some(parent) = node_stack.last_mut() {
                        let child_index = parent.add_child(node);
                        node.parent = Some(child_index);
                        node_stack.push(parent.children.get_mut(child_index).unwrap());
                        parent_stack.push(child_index);
                    }
                }
                Some(FdtToken::EndNode) => {
                    node_stack.pop();
                    parent_stack.pop();
                }
                Some(FdtToken::Prop) => {
                    // Read property
                    let (prop, prop_len) = self.read_property_at(current_offset, strings_offset)?;
                    current_offset = align_up(current_offset + prop_len, 4);

                    if let Some(node) = node_stack.last_mut() {
                        node.add_property(prop);
                    }
                }
                Some(FdtToken::Nop) => {
                    // No operation
                }
                Some(FdtToken::End) => {
                    // End of structure block
                    break;
                }
                None => {
                    return Err("Invalid FDT token");
                }
            }
        }

        Ok(())
    }

    /// Read string at offset
    fn read_string_at(&self, offset: usize) -> Result<(&str, usize), &'static str> {
        let mut len = 0;
        let mut end = offset;

        while end < self.data.len() && self.data[end] != 0 {
            len += 1;
            end += 1;
        }

        if end >= self.data.len() {
            return Err("String not null terminated");
        }

        let string_bytes = &self.data[offset..end];
        let string = str::from_utf8(string_bytes)
            .map_err(|_| "Invalid UTF-8 string")?;

        Ok((string, len))
    }

    /// Read property at offset
    fn read_property_at(
        &self,
        struct_offset: usize,
        strings_offset: usize,
    ) -> Result<(Property, usize), &'static str> {
        // Read name offset and length
        if struct_offset + 8 > self.data.len() {
            return Err("Property header too short");
        }

        let name_offset = u32::from_be_bytes([
            self.data[struct_offset],
            self.data[struct_offset + 1],
            self.data[struct_offset + 2],
            self.data[struct_offset + 3],
        ]) as usize;

        let prop_len = u32::from_be_bytes([
            self.data[struct_offset + 4],
            self.data[struct_offset + 5],
            self.data[struct_offset + 6],
            self.data[struct_offset + 7],
        ]) as usize;

        // Read name from strings block
        if strings_offset + name_offset >= self.data.len() {
            return Err("Name offset out of bounds");
        }

        let (name, _) = self.read_string_at(strings_offset + name_offset)?;

        // Read property data
        if struct_offset + 8 + prop_len > self.data.len() {
            return Err("Property data out of bounds");
        }

        let data = self.data[struct_offset + 8..struct_offset + 8 + prop_len].to_vec();

        let prop = Property::new(name, data);
        let total_len = 8 + prop_len;

        Ok((prop, total_len))
    }

    /// Get root node
    pub fn get_root(&self) -> Option<&Node> {
        self.root.as_ref()
    }

    /// Get mutable root node
    pub fn get_root_mut(&mut self) -> Option<&mut Node> {
        self.root.as_mut()
    }

    /// Find node by path
    pub fn find_node(&self, path: &str) -> Option<&Node> {
        self.root.as_ref()?.find_path(path)
    }

    /// Find node by path (mutable)
    pub fn find_node_mut(&mut self, path: &str) -> Option<&mut Node> {
        self.root.as_mut()?.find_path_mut(path)
    }

    /// Get property by path
    pub fn get_property(&self, node_path: &str, prop_name: &str) -> Option<&Property> {
        self.find_node(node_path)?.get_property(prop_name)
    }

    /// Get memory reserve entries
    pub fn get_mem_reserve(&self) -> &[MemReserveEntry] {
        &self.mem_reserve
    }

    /// Get boot CPU ID
    pub fn get_boot_cpu_id(&self) -> u32 {
        self.header.boot_cpuid_phys
    }

    /// Serialize back to bytes
    pub fn serialize(&self) -> Result<Vec<u8>, &'static str> {
        // This would serialize the tree structure back to FDT format
        // For now, return the original data
        Ok(self.data.clone())
    }
}

/// Align value up to alignment
fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

/// Device tree iterator
pub struct NodeIterator<'a> {
    stack: Vec<&'a Node>,
}

impl<'a> NodeIterator<'a> {
    /// Create new iterator
    pub fn new(root: &'a Node) -> Self {
        Self { stack: vec![root] }
    }
}

impl<'a> Iterator for NodeIterator<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.stack.pop() {
            // Push children in reverse order
            for child in node.children.iter().rev() {
                self.stack.push(child);
            }
            Some(node)
        } else {
            None
        }
    }
}

/// Property iterator
pub struct PropertyIterator<'a> {
    nodes: NodeIterator<'a>,
    current_props: Option<std::slice::Iter<'a, Property>>,
    current_node: Option<&'a Node>,
}

impl<'a> PropertyIterator<'a> {
    /// Create new property iterator
    pub fn new(root: &'a Node) -> Self {
        let mut iter = Self {
            nodes: NodeIterator::new(root),
            current_props: None,
            current_node: None,
        };
        iter.advance();
        iter
    }

    fn advance(&mut self) {
        while let Some(node) = self.nodes.next() {
            if !node.properties.is_empty() {
                self.current_props = Some(node.properties.iter());
                self.current_node = Some(node);
                break;
            }
        }
    }
}

impl<'a> Iterator for PropertyIterator<'a> {
    type Item = (&'a Node, &'a Property);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut props) = self.current_props {
            if let Some(prop) = props.next() {
                return Some((self.current_node.unwrap(), prop));
            }
        }

        // Move to next node
        self.current_props = None;
        self.advance();
        self.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fdt_header() {
        let header = FdtHeader {
            magic: 0xd00dfeed,
            totalsize: 1024,
            off_dt_struct: 256,
            off_dt_strings: 768,
            off_mem_rsvmap: 48,
            version: 17,
            last_comp_version: 16,
            boot_cpuid_phys: 0,
            size_dt_strings: 128,
            size_dt_struct: 256,
        };

        assert!(header.is_valid());
        assert_eq!(header.get_struct_block_offset(), 256);
        assert_eq!(header.get_strings_block_offset(), 768);
    }

    #[test]
    fn test_fdt_token() {
        assert_eq!(FdtToken::from_u32(0x1), Some(FdtToken::BeginNode));
        assert_eq!(FdtToken::from_u32(0x2), Some(FdtToken::EndNode));
        assert_eq!(FdtToken::from_u32(0x3), Some(FdtToken::Prop));
        assert_eq!(FdtToken::from_u32(0x9), Some(FdtToken::End));
        assert_eq!(FdtToken::from_u32(0xFF), None);
    }

    #[test]
    fn test_mem_reserve_entry() {
        let entry = MemReserveEntry::new(0x80000000, 0x100000);
        assert_eq!(entry.address, 0x80000000);
        assert_eq!(entry.size, 0x100000);
        assert!(!entry.is_end());

        let end_entry = MemReserveEntry::new(0, 0);
        assert!(end_entry.is_end());
    }

    #[test]
    fn test_property() {
        let prop = Property::new("test", vec![0x12, 0x34, 0x56, 0x78]);
        assert_eq!(prop.name, "test");
        assert_eq!(prop.as_u32(), Some(0x12345678));
        assert_eq!(prop.prop_type, PropertyType::U32);

        let string_prop = Property::new("string", b"hello\0");
        assert_eq!(string_prop.as_string(), Some("hello"));
        assert_eq!(string_prop.prop_type, PropertyType::String);
    }

    #[test]
    fn test_node() {
        let mut node = Node::new("test-node", 1);

        node.add_property(Property::new("prop1", vec![1, 2, 3, 4]));
        assert!(node.get_property("prop1").is_some());
        assert_eq!(node.get_prop_u32("prop1"), Some(0x01020304));

        let child = Node::new("child", 2);
        let _ = node.add_child(child);
        assert!(node.find_child("child").is_some());
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(1, 4), 4);
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(align_up(7, 8), 8);
    }
}