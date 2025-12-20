//! Intrusive linked list implementation
//!
//! This module provides a simple intrusive linked list implementation
//! suitable for use in the kernel environment without heap allocation.

use core::ptr::{self, NonNull};

/// A node in an intrusive linked list
#[derive(Debug)]
pub struct ListNode {
    /// Next node in the list
    next: Option<NonNull<ListNode>>,
    /// Previous node in the list
    prev: Option<NonNull<ListNode>>,
}

impl ListNode {
    /// Create a new list node
    pub const fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }

    /// Check if the node is linked in a list
    pub fn is_linked(&self) -> bool {
        self.next.is_some() || self.prev.is_some()
    }
}

/// An intrusive linked list
#[derive(Debug)]
pub struct List {
    /// Head of the list
    head: Option<NonNull<ListNode>>,
    /// Tail of the list
    tail: Option<NonNull<ListNode>>,
    /// Length of the list
    len: usize,
}

impl List {
    /// Create a new empty list
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// Get the length of the list
    pub fn len(&self) -> usize {
        self.len
    }

    /// Get the first node
    pub fn front(&self) -> Option<&ListNode> {
        self.head.map(|node| unsafe { node.as_ref() })
    }

    /// Get the last node
    pub fn back(&self) -> Option<&ListNode> {
        self.tail.map(|node| unsafe { node.as_ref() })
    }

    /// Push a node to the front of the list
    ///
    /// # Safety
    /// The node must not already be in a list
    pub unsafe fn push_front(&mut self, node: NonNull<ListNode>) {
        debug_assert!(!node.as_ref().is_linked(), "Node is already linked");

        match self.head {
            None => {
                // Empty list
                self.head = Some(node);
                self.tail = Some(node);
                node.as_ref().next = None;
                node.as_ref().prev = None;
            }
            Some(head) => {
                node.as_ref().next = Some(head);
                node.as_ref().prev = None;
                head.as_ref().prev = Some(node);
                self.head = Some(node);
            }
        }
        self.len += 1;
    }

    /// Push a node to the back of the list
    ///
    /// # Safety
    /// The node must not already be in a list
    pub unsafe fn push_back(&mut self, node: NonNull<ListNode>) {
        debug_assert!(!node.as_ref().is_linked(), "Node is already linked");

        match self.tail {
            None => {
                // Empty list
                self.head = Some(node);
                self.tail = Some(node);
                node.as_ref().next = None;
                node.as_ref().prev = None;
            }
            Some(tail) => {
                node.as_ref().next = None;
                node.as_ref().prev = Some(tail);
                tail.as_ref().next = Some(node);
                self.tail = Some(node);
            }
        }
        self.len += 1;
    }

    /// Pop a node from the front of the list
    pub fn pop_front(&mut self) -> Option<NonNull<ListNode>> {
        self.head.map(|head| {
            unsafe {
                self.head = head.as_ref().next;
                self.len -= 1;

                if let Some(new_head) = self.head {
                    new_head.as_ref().prev = None;
                } else {
                    self.tail = None;
                }

                head.as_mut().next = None;
                head.as_mut().prev = None;
            }
            head
        })
    }

    /// Pop a node from the back of the list
    pub fn pop_back(&mut self) -> Option<NonNull<ListNode>> {
        self.tail.map(|tail| {
            unsafe {
                self.tail = tail.as_ref().prev;
                self.len -= 1;

                if let Some(new_tail) = self.tail {
                    new_tail.as_ref().next = None;
                } else {
                    self.head = None;
                }

                tail.as_mut().next = None;
                tail.as_mut().prev = None;
            }
            tail
        })
    }

    /// Remove a specific node from the list
    ///
    /// # Safety
    /// The node must be in this list
    pub unsafe fn remove(&mut self, node: NonNull<ListNode>) -> bool {
        let node_ref = node.as_ref();
        if !node_ref.is_linked() {
            return false;
        }

        if let Some(prev) = node_ref.prev {
            prev.as_ref().next = node_ref.next;
        } else {
            // Node is head
            self.head = node_ref.next;
        }

        if let Some(next) = node_ref.next {
            next.as_ref().prev = node_ref.prev;
        } else {
            // Node is tail
            self.tail = node_ref.prev;
        }

        node.as_mut().next = None;
        node.as_mut().prev = None;
        self.len -= 1;
        true
    }

    /// Insert a node after the given node
    ///
    /// # Safety
    /// The new node must not already be in a list
    /// The existing node must be in this list
    pub unsafe fn insert_after(&mut self, existing: NonNull<ListNode>, new_node: NonNull<ListNode>) {
        debug_assert!(!new_node.as_ref().is_linked(), "New node is already linked");
        debug_assert!(existing.as_ref().is_linked(), "Existing node is not linked");

        let existing_ref = existing.as_ref();
        let next = existing_ref.next;

        new_node.as_ref().next = next;
        new_node.as_ref().prev = Some(existing);
        existing_ref.next = Some(new_node);

        if let Some(next) = next {
            next.as_ref().prev = Some(new_node);
        } else {
            // New node is now the tail
            self.tail = Some(new_node);
        }
        self.len += 1;
    }

    /// Insert a node before the given node
    ///
    /// # Safety
    /// The new node must not already be in a list
    /// The existing node must be in this list
    pub unsafe fn insert_before(&mut self, existing: NonNull<ListNode>, new_node: NonNull<ListNode>) {
        debug_assert!(!new_node.as_ref().is_linked(), "New node is already linked");
        debug_assert!(existing.as_ref().is_linked(), "Existing node is not linked");

        let existing_ref = existing.as_ref();
        let prev = existing_ref.prev;

        new_node.as_ref().next = Some(existing);
        new_node.as_ref().prev = prev;
        existing_ref.prev = Some(new_node);

        if let Some(prev) = prev {
            prev.as_ref().next = Some(new_node);
        } else {
            // New node is now the head
            self.head = Some(new_node);
        }
        self.len += 1;
    }

    /// Clear the list, removing all nodes
    pub fn clear(&mut self) {
        while let Some(node) = self.pop_front() {
            unsafe {
                node.as_mut().next = None;
                node.as_mut().prev = None;
            }
        }
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over nodes in a list
pub struct Iter<'a> {
    current: Option<NonNull<ListNode>>,
    _phantom: core::marker::PhantomData<&'a ()>,
}

impl<'a> List {
    /// Create an iterator over the list
    pub fn iter(&'a self) -> Iter<'a> {
        Iter {
            current: self.head,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ListNode;

    fn next(&mut self) -> Option<Self::Item> {
        self.current.map(|node| {
            unsafe {
                let node_ref = node.as_ref();
                self.current = node_ref.next;
                node_ref
            }
        })
    }
}

/// Macro to implement intrusive list for a struct
#[macro_export]
macro_rules! impl_list_node {
    ($struct_name:ident, $node_field:ident) => {
        impl $struct_name {
            /// Create a new instance with an uninitialized list node
            pub fn new() -> Self {
                Self {
                    $node_field: $crate::utils::list::ListNode::new(),
                    // Add other field initializations here
                    ..Default::default()
                }
            }

            /// Get a reference to the list node
            pub fn as_list_node(&self) -> &$crate::utils::list::ListNode {
                &self.$node_field
            }

            /// Get a mutable reference to the list node
            pub fn as_list_node_mut(&mut self) -> &mut $crate::utils::list::ListNode {
                &mut self.$node_field
            }

            /// Check if this node is linked in a list
            pub fn is_linked(&self) -> bool {
                self.$node_field.is_linked()
            }
        }

        impl From<*mut $crate::utils::list::ListNode> for *mut $struct_name {
            fn from(node: *mut $crate::utils::list::ListNode) -> Self {
                // Use offset to get back to the struct
                let offset = ::core::mem::offset_of!($struct_name, $node_field);
                unsafe {
                    (node as *mut u8).sub(offset) as *mut $struct_name
                }
            }
        }

        impl From<*mut $struct_name> for *mut $crate::utils::list::ListNode {
            fn from(ptr: *mut $struct_name) -> Self {
                unsafe {
                    ptr.add(::core::mem::offset_of!($struct_name, $node_field)) as *mut $crate::utils::list::ListNode
                }
            }
        }
    };
}