//! Slab allocator
//!
//! Provides efficient allocation for fixed-size objects.

use crate::core::mm::{VirtAddr, PAGE_SIZE};
use crate::utils::spinlock::SpinLock;
use crate::utils::bitmap::Bitmap;
use core::ptr::NonNull;

/// Slab cache for fixed-size objects
pub struct SlabCache {
    /// Name of this slab cache
    name: &'static str,
    /// Size of objects in this cache
    object_size: usize,
    /// Alignment for objects
    align: usize,
    /// Number of objects per slab
    objects_per_slab: usize,
    /// List of partial slabs (with some free objects)
    partial_slabs: SpinLock<SlabList>,
    /// List of full slabs (no free objects)
    full_slabs: SpinLock<SlabList>,
    /// List of empty slabs (all objects free)
    empty_slabs: SpinLock<SlabList>,
    /// Statistics
    stats: SpinLock<SlabStats>,
}

/// A slab containing multiple objects
pub struct Slab {
    /// Start address of the slab
    start_addr: VirtAddr,
    /// Number of objects in this slab
    object_count: usize,
    /// Bitmap tracking which objects are free
    free_bitmap: Bitmap,
    /// Number of free objects
    free_count: usize,
    /// Next slab in list
    next: Option<NonNull<Slab>>,
    /// Previous slab in list
    prev: Option<NonNull<Slab>>,
}

/// Linked list of slabs
pub struct SlabList {
    head: Option<NonNull<Slab>>,
    tail: Option<NonNull<Slab>>,
    count: usize,
}

/// Slab cache statistics
#[derive(Debug, Clone, Copy)]
pub struct SlabStats {
    /// Total number of objects allocated
    pub total_allocated: usize,
    /// Current number of objects in use
    pub objects_in_use: usize,
    /// Total number of slabs allocated
    pub total_slabs: usize,
    /// Number of slabs with free objects
    pub partial_slabs: usize,
    /// Number of full slabs
    pub full_slabs: usize,
    /// Number of empty slabs
    pub empty_slabs: usize,
}

impl SlabList {
    /// Create a new empty slab list
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            count: 0,
        }
    }

    /// Add a slab to the front of the list
    pub fn push_front(&mut self, slab: NonNull<Slab>) {
        unsafe {
            let slab_mut = slab.as_mut();
            slab_mut.next = self.head;
            slab_mut.prev = None;

            if let Some(head) = self.head {
                head.as_mut().prev = Some(slab);
            } else {
                self.tail = Some(slab);
            }

            self.head = Some(slab);
            self.count += 1;
        }
    }

    /// Add a slab to the end of the list
    pub fn push_back(&mut self, slab: NonNull<Slab>) {
        unsafe {
            let slab_mut = slab.as_mut();
            slab_mut.next = None;
            slab_mut.prev = self.tail;

            if let Some(tail) = self.tail {
                tail.as_mut().next = Some(slab);
            } else {
                self.head = Some(slab);
            }

            self.tail = Some(slab);
            self.count += 1;
        }
    }

    /// Remove a slab from the front of the list
    pub fn pop_front(&mut self) -> Option<NonNull<Slab>> {
        self.head.map(|head| {
            unsafe {
                let head_mut = head.as_mut();
                self.head = head_mut.next;

                if let Some(next) = head_mut.next {
                    next.as_mut().prev = None;
                } else {
                    self.tail = None;
                }

                head_mut.next = None;
                head_mut.prev = None;
                self.count -= 1;
            }
            head
        })
    }

    /// Remove a slab from the end of the list
    pub fn pop_back(&mut self) -> Option<NonNull<Slab>> {
        self.tail.map(|tail| {
            unsafe {
                let tail_mut = tail.as_mut();
                self.tail = tail_mut.prev;

                if let Some(prev) = tail_mut.prev {
                    prev.as_mut().next = None;
                } else {
                    self.head = None;
                }

                tail_mut.next = None;
                tail_mut.prev = None;
                self.count -= 1;
            }
            tail
        })
    }

    /// Remove a specific slab from the list
    pub fn remove(&mut self, slab: NonNull<Slab>) -> bool {
        unsafe {
            let slab_ref = slab.as_ref();

            if let Some(prev) = slab_ref.prev {
                prev.as_mut().next = slab_ref.next;
            } else {
                // Slab is at head
                self.head = slab_ref.next;
            }

            if let Some(next) = slab_ref.next {
                next.as_mut().prev = slab_ref.prev;
            } else {
                // Slab is at tail
                self.tail = slab_ref.prev;
            }

            let slab_mut = slab.as_mut();
            slab_mut.next = None;
            slab_mut.prev = None;
            self.count -= 1;
        }
        true
    }

    /// Get the number of slabs in the list
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl Slab {
    /// Create a new slab
    pub fn new(
        start_addr: VirtAddr,
        object_size: usize,
        objects_per_slab: usize,
    ) -> Option<NonNull<Self>> {
        // Calculate the bitmap size needed
        let bitmap_size = (objects_per_slab + 63) / 64;
        let bitmap_data = start_addr as *mut u64;

        // Calculate object data start address
        let object_data_start = (start_addr as usize + bitmap_size * 8 + object_size - 1)
            & !(object_size - 1);

        let slab_ptr = start_addr as *mut Self;
        let slab = unsafe { &mut *slab_ptr };

        slab.start_addr = start_addr;
        slab.object_count = objects_per_slab;
        slab.free_bitmap = unsafe {
            Bitmap::new(bitmap_data, objects_per_slab)
        };
        slab.free_count = objects_per_slab;
        slab.next = None;
        slab.prev = None;

        // Mark all objects as free
        unsafe {
            slab.free_bitmap.clear_all();
        }

        Some(unsafe { NonNull::new_unchecked(slab_ptr) })
    }

    /// Get the start address of the slab
    pub fn start_addr(&self) -> VirtAddr {
        self.start_addr
    }

    /// Get the number of free objects
    pub fn free_count(&self) -> usize {
        self.free_count
    }

    /// Check if the slab is empty (all objects free)
    pub fn is_empty(&self) -> bool {
        self.free_count == self.object_count
    }

    /// Check if the slab is full (no free objects)
    pub fn is_full(&self) -> bool {
        self.free_count == 0
    }

    /// Allocate an object from the slab
    pub fn allocate(&mut self) -> Option<VirtAddr> {
        if self.free_count == 0 {
            return None;
        }

        // Find a free object
        if let Some(index) = self.free_bitmap.find_and_set() {
            self.free_count -= 1;

            // Calculate object address
            let bitmap_size = (self.object_count + 63) / 64;
            let object_data_start = (self.start_addr as usize + bitmap_size * 8)
                & !(self.object_count * self.object_size - 1);
            let object_addr = object_data_start + index * self.object_size;

            Some(object_addr as u64)
        } else {
            None
        }
    }

    /// Deallocate an object to the slab
    pub fn deallocate(&mut self, addr: VirtAddr) -> bool {
        // Calculate object index
        let bitmap_size = (self.object_count + 63) / 64;
        let object_data_start = (self.start_addr as usize + bitmap_size * 8)
            & !(self.object_count * self.object_size - 1);

        if addr < object_data_start as u64 {
            return false;
        }

        let offset = (addr - object_data_start as u64) as usize;
        let index = offset / (object_data_start / self.object_count);

        if index >= self.object_count {
            return false;
        }

        // Check if the object is currently allocated
        if !self.free_bitmap.test(index) {
            return false; // Already free
        }

        // Free the object
        self.free_bitmap.clear_bit(index);
        self.free_count += 1;

        true
    }
}

impl SlabCache {
    /// Create a new slab cache
    pub fn new(
        name: &'static str,
        object_size: usize,
        align: usize,
    ) -> Self {
        // Align object size to at least 16 bytes
        let aligned_size = if object_size < 16 {
            16
        } else {
            (object_size + align - 1) & !(align - 1)
        };

        // Calculate how many objects fit in a slab
        let header_size = core::mem::size_of::<Slab>();
        let available_space = PAGE_SIZE - header_size;
        let objects_per_slab = available_space / (aligned_size + 1); // +1 for bitmap bit

        Self {
            name,
            object_size: aligned_size,
            align,
            objects_per_slab,
            partial_slabs: SpinLock::new(SlabList::new()),
            full_slabs: SpinLock::new(SlabList::new()),
            empty_slabs: SpinLock::new(SlabList::new()),
            stats: SpinLock::new(SlabStats {
                total_allocated: 0,
                objects_in_use: 0,
                total_slabs: 0,
                partial_slabs: 0,
                full_slabs: 0,
                empty_slabs: 0,
            }),
        }
    }

    /// Get the name of this slab cache
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Get the object size
    pub fn object_size(&self) -> usize {
        self.object_size
    }

    /// Allocate an object from the cache
    pub fn allocate(&self) -> Option<NonNull<u8>> {
        // Try partial slabs first
        if let Some(slab_ptr) = self.partial_slabs.lock().pop_front() {
            let slab = unsafe { slab_ptr.as_mut() };
            if let Some(obj_addr) = slab.allocate() {
                // Update statistics
                {
                    let mut stats = self.stats.lock();
                    stats.total_allocated += 1;
                    stats.objects_in_use += 1;
                    stats.partial_slabs = self.partial_slabs.lock().count;
                    stats.full_slabs = self.full_slabs.lock().count;
                }

                // Return slab to appropriate list
                if slab.is_full() {
                    self.full_slabs.lock().push_front(slab_ptr);
                } else {
                    self.partial_slabs.lock().push_front(slab_ptr);
                }

                return Some(unsafe { NonNull::new_unchecked(obj_addr as *mut u8) });
            }
        }

        // Try to get an empty slab
        if let Some(slab_ptr) = self.empty_slabs.lock().pop_front() {
            let slab = unsafe { slab_ptr.as_mut() };
            if let Some(obj_addr) = slab.allocate() {
                // Update statistics
                {
                    let mut stats = self.stats.lock();
                    stats.total_allocated += 1;
                    stats.objects_in_use += 1;
                    stats.empty_slabs = self.empty_slabs.lock().count;
                    stats.partial_slabs = self.partial_slabs.lock().count;
                }

                self.partial_slabs.lock().push_front(slab_ptr);
                return Some(unsafe { NonNull::new_unchecked(obj_addr as *mut u8) });
            }
        }

        // Need to allocate a new slab
        // TODO: Allocate new slab from page allocator
        None
    }

    /// Deallocate an object to the cache
    pub fn deallocate(&self, obj: NonNull<u8>) -> bool {
        let obj_addr = obj.as_ptr() as VirtAddr;

        // Try to find the slab containing this object
        // In a real implementation, we'd use metadata to locate the slab faster

        // Check partial slabs
        {
            let mut partial_slabs = self.partial_slabs.lock();
            let mut current = partial_slabs.head;

            while let Some(slab_ptr) = current {
                let slab = unsafe { slab_ptr.as_ref() };
                if obj_addr >= slab.start_addr() && obj_addr < slab.start_addr() + PAGE_SIZE as u64 {
                    // Found the slab
                    drop(partial_slabs);
                    let slab_mut = unsafe { slab_ptr.as_mut() };
                    if slab_mut.deallocate(obj_addr) {
                        // Update statistics
                        {
                            let mut stats = self.stats.lock();
                            stats.objects_in_use -= 1;
                        }

                        // Move slab to appropriate list
                        if slab_mut.is_empty() {
                            self.partial_slabs.lock().remove(slab_ptr);
                            self.empty_slabs.lock().push_front(slab_ptr);
                        }

                        return true;
                    }
                    return false;
                }
                current = slab.next;
            }
        }

        // Check full slabs
        {
            let mut full_slabs = self.full_slabs.lock();
            let mut current = full_slabs.head;

            while let Some(slab_ptr) = current {
                let slab = unsafe { slab_ptr.as_ref() };
                if obj_addr >= slab.start_addr() && obj_addr < slab.start_addr() + PAGE_SIZE as u64 {
                    // Found the slab
                    drop(full_slabs);
                    let slab_mut = unsafe { slab_ptr.as_mut() };
                    if slab_mut.deallocate(obj_addr) {
                        // Update statistics
                        {
                            let mut stats = self.stats.lock();
                            stats.objects_in_use -= 1;
                        }

                        // Move slab to appropriate list
                        self.full_slabs.lock().remove(slab_ptr);
                        self.partial_slabs.lock().push_front(slab_ptr);

                        return true;
                    }
                    return false;
                }
                current = slab.next;
            }
        }

        false
    }

    /// Get cache statistics
    pub fn stats(&self) -> SlabStats {
        let mut stats = self.stats.lock();
        stats.partial_slabs = self.partial_slabs.lock().count;
        stats.full_slabs = self.full_slabs.lock().count;
        stats.empty_slabs = self.empty_slabs.lock().count;
        *stats
    }
}

/// Initialize the slab allocator
pub fn init() -> Result<(), crate::Error> {
    // TODO: Initialize common slab caches
    Err(crate::Error::NotImplemented)
}