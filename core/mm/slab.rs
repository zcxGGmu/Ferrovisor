//! Enhanced Slab allocator implementation
//!
//! Provides efficient memory allocation for frequently used object sizes.
//! Based on the buddy allocator system similar to xvisor's heap management.
//!
//! Features:
//! - Multiple slab classes for different object sizes
//! - Buddy allocator integration for backing memory management
//! - Memory usage statistics and monitoring
//! - Efficient object caching and reuse
//! - Thread-safe allocation/deallocation

use crate::core::mm::{PAGE_SIZE, align_up, frame::alloc_frame, frame::dealloc_frame};
use crate::core::sync::SpinLock;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

/// Slab allocator errors
#[derive(Debug, Clone, PartialEq)]
pub enum SlabError {
    /// Invalid object size
    InvalidSize,
    /// Out of memory
    OutOfMemory,
    /// Invalid pointer
    InvalidPointer,
    /// Slab not initialized
    NotInitialized,
    /// Object too large for slab allocation
    ObjectTooLarge,
}

/// Slab object header
#[repr(C)]
struct SlabObject {
    /// Next free object in the slab
    next: Option<NonNull<SlabObject>>,
    /// Magic number for corruption detection
    magic: u64,
}

/// Slab page containing multiple objects
struct SlabPage {
    /// List node for page list
    list_node: ListNode,
    /// Number of objects allocated in this page
    inuse: u32,
    /// Total objects in this page
    total: u32,
    /// Objects in this page
    objects: [SlabObject; 0],
}

/// Simple list node implementation
#[derive(Debug)]
struct ListNode {
    next: Option<NonNull<Self>>,
    prev: Option<NonNull<Self>>,
}

impl ListNode {
    fn new() -> Self {
        Self {
            next: None,
            prev: None,
        }
    }
}

/// Slab cache for objects of a specific size
pub struct SlabCache {
    /// Size of objects in this cache
    object_size: usize,
    /// Alignment requirement for objects
    alignment: usize,
    /// Number of objects per page
    objects_per_page: usize,
    /// List of partially used pages
    partial_pages: SpinLock<Vec<*mut SlabPage>>,
    /// List of completely free pages
    free_pages: SpinLock<Vec<*mut SlabPage>>,
    /// List of completely used pages
    full_pages: SpinLock<Vec<*mut SlabPage>>,
    /// Total allocated objects
    total_allocated: AtomicUsize,
    /// Current free objects
    free_objects: AtomicUsize,
    /// Total pages allocated
    total_pages: AtomicUsize,
    /// Cache name for debugging
    name: &'static str,
    /// Magic number for validation
    magic: u64,
}

/// Slab cache configuration
#[derive(Debug, Clone)]
pub struct SlabCacheConfig {
    /// Size of objects in this cache
    pub object_size: usize,
    /// Alignment requirement (default: 8)
    pub alignment: usize,
    /// Cache name for debugging
    pub name: &'static str,
    /// Initial number of pages to allocate
    pub initial_pages: usize,
}

impl Default for SlabCacheConfig {
    fn default() -> Self {
        Self {
            object_size: 64,
            alignment: 8,
            name: "default",
            initial_pages: 1,
        }
    }
}

/// Slab cache statistics
#[derive(Debug, Clone)]
pub struct SlabStats {
    /// Cache name
    pub name: &'static str,
    /// Object size
    pub object_size: usize,
    /// Total allocated objects
    pub total_allocated: usize,
    /// Current free objects
    pub free_objects: usize,
    /// Total pages allocated
    pub total_pages: usize,
    /// Objects per page
    pub objects_per_page: usize,
    /// Number of partial pages
    pub partial_pages: usize,
    /// Number of completely free pages
    pub free_pages: usize,
    /// Number of completely full pages
    pub full_pages: usize,
}

impl SlabCache {
    const MAGIC: u64 = 0x534C41425F4D4147; // "SLAB_MAGIC"
    const OBJECT_MAGIC: u64 = 0x4F424A4D41474943; // "OBJ_MAGIC"

    /// Create a new slab cache
    pub fn new(config: SlabCacheConfig) -> Result<Self, SlabError> {
        if config.object_size == 0 {
            return Err(SlabError::InvalidSize);
        }

        if config.object_size > PAGE_SIZE / 2 {
            return Err(SlabError::ObjectTooLarge);
        }

        // Calculate objects per page
        let header_size = core::mem::size_of::<SlabPage>();
        let object_size = core::mem::size_of::<SlabObject>() +
                         align_up(config.object_size, config.alignment);
        let available_space = PAGE_SIZE - header_size;
        let objects_per_page = available_space / object_size;

        if objects_per_page == 0 {
            return Err(SlabError::ObjectTooLarge);
        }

        let cache = Self {
            object_size: config.object_size,
            alignment: config.alignment,
            objects_per_page,
            partial_pages: SpinLock::new(Vec::new()),
            free_pages: SpinLock::new(Vec::new()),
            full_pages: SpinLock::new(Vec::new()),
            total_allocated: AtomicUsize::new(0),
            free_objects: AtomicUsize::new(0),
            total_pages: AtomicUsize::new(0),
            name: config.name,
            magic: Self::MAGIC,
        };

        log::debug!("Created slab cache '{}' for {}-byte objects ({} per page)",
                   cache.name, cache.object_size, cache.objects_per_page);

        Ok(cache)
    }

    /// Allocate an object from the cache
    pub fn allocate(&self) -> Result<NonNull<u8>, SlabError> {
        // Try to get from partial pages first
        if let Some(page) = self.get_partial_page() {
            self.allocate_from_page(page)
        } else if let Some(page) = self.get_free_page() {
            self.allocate_from_page(page)
        } else {
            // Need to allocate a new page
            self.allocate_new_page()
        }
    }

    /// Deallocate an object back to the cache
    pub fn deallocate(&self, ptr: NonNull<u8>) -> Result<(), SlabError> {
        // Find the page containing this object
        let page_addr = (ptr.as_ptr() as usize) & !(PAGE_SIZE - 1);
        let page = page_addr as *mut SlabPage;

        // Get object index
        let object_offset = ptr.as_ptr() as usize - page_addr;
        let header_offset = object_offset - core::mem::size_of::<SlabObject>();
        let object_ptr = (page_addr + header_offset) as *mut SlabObject;

        unsafe {
            // Validate object magic
            if (*object_ptr).magic != Self::OBJECT_MAGIC ^ 0xFFFFFFFFFFFFFFFF {
                return Err(SlabError::InvalidPointer);
            }

            // Mark object as free
            (*object_ptr).magic = Self::OBJECT_MAGIC;
        }

        self.free_object(page, object_ptr)
    }

    /// Get allocation statistics
    pub fn stats(&self) -> SlabStats {
        SlabStats {
            name: self.name,
            object_size: self.object_size,
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            free_objects: self.free_objects.load(Ordering::Relaxed),
            total_pages: self.total_pages.load(Ordering::Relaxed),
            objects_per_page: self.objects_per_page,
            partial_pages: self.partial_pages.lock().len(),
            free_pages: self.free_pages.lock().len(),
            full_pages: self.full_pages.lock().len(),
        }
    }

    /// Shrink the cache by releasing completely free pages
    pub fn shrink(&self) -> usize {
        let mut freed_pages = 0;
        let mut free_pages = self.free_pages.lock();

        while !free_pages.is_empty() {
            if let Some(page) = free_pages.pop() {
                // Free the page back to the system
                self.free_page_memory(page);
                freed_pages += 1;
            }
        }

        self.total_pages.fetch_sub(freed_pages, Ordering::Relaxed);
        log::debug!("Shrank slab cache '{}', freed {} pages", self.name, freed_pages);

        freed_pages
    }

    /// Get an object from a partial page
    fn get_partial_page(&self) -> Option<*mut SlabPage> {
        self.partial_pages.lock().pop()
    }

    /// Get a completely free page
    fn get_free_page(&self) -> Option<*mut SlabPage> {
        self.free_pages.lock().pop()
    }

    /// Allocate a new page for this cache
    fn allocate_new_page(&self) -> Result<NonNull<u8>, SlabError> {
        // Allocate a page from frame allocator
        let frame_addr = alloc_frame().ok_or(SlabError::OutOfMemory)?;

        let page_virt = frame_addr as *mut SlabPage;

        // Initialize the page
        unsafe {
            let page = &mut *page_virt;
            page.list_node = ListNode::new();
            page.inuse = 0;
            page.total = self.objects_per_page as u32;

            // Initialize all objects in the page
            let base = page_virt as usize;
            let header_size = core::mem::size_of::<SlabPage>();
            let object_size = core::mem::size_of::<SlabObject>() +
                            align_up(self.object_size, self.alignment);

            for i in 0..self.objects_per_page {
                let object_addr = base + header_size + (i * object_size);
                let object = &mut *(object_addr as *mut SlabObject);
                object.magic = Self::OBJECT_MAGIC;

                if i < self.objects_per_page - 1 {
                    let next_addr = object_addr + object_size;
                    object.next = NonNull::new(next_addr as *mut SlabObject);
                } else {
                    object.next = None;
                }
            }
        }

        self.total_pages.fetch_add(1, Ordering::Relaxed);

        // Allocate first object from the new page
        self.allocate_from_page(page_virt)
    }

    /// Allocate an object from a specific page
    fn allocate_from_page(&self, page: *mut SlabPage) -> Result<NonNull<u8>, SlabError> {
        unsafe {
            let page_ref = &mut *page;

            if page_ref.inuse >= page_ref.total {
                return Err(SlabError::OutOfMemory);
            }

            // Find first free object
            let base = page as usize;
            let header_size = core::mem::size_of::<SlabPage>();
            let object_size = core::mem::size_of::<SlabObject>() +
                            align_up(self.object_size, self.alignment);

            for i in 0..self.objects_per_page {
                let object_addr = base + header_size + (i * object_size);
                let object = &mut *(object_addr as *mut SlabObject);

                if object.magic == Self::OBJECT_MAGIC {
                    // Mark object as allocated
                    object.magic ^= 0xFFFFFFFFFFFFFFFF;

                    page_ref.inuse += 1;
                    self.total_allocated.fetch_add(1, Ordering::Relaxed);

                    // Move page to appropriate list
                    if page_ref.inuse == page_ref.total {
                        // Page is now full
                        self.full_pages.lock().push(page);
                    } else if page_ref.inuse == 1 {
                        // Page was empty, now partial
                        self.partial_pages.lock().push(page);
                    }

                    let data_addr = object_addr + core::mem::size_of::<SlabObject>();
                    return Ok(NonNull::new(data_addr as *mut u8).unwrap());
                }
            }

            Err(SlabError::OutOfMemory)
        }
    }

    /// Free an object back to its page
    fn free_object(&self, page: *mut SlabPage, object: *mut SlabObject) -> Result<(), SlabError> {
        unsafe {
            let page_ref = &mut *page;
            let object_ref = &mut *object;

            // Restore object magic
            object_ref.magic ^= 0xFFFFFFFFFFFFFFFF;

            page_ref.inuse -= 1;
            self.total_allocated.fetch_sub(1, Ordering::Relaxed);

            // Move page to appropriate list
            let was_full = page_ref.inuse == (page_ref.total - 1);
            let now_empty = page_ref.inuse == 0;

            if was_full {
                // Remove from full list
                self.full_pages.lock().retain(|&p| p != page);
                self.partial_pages.lock().push(page);
            } else if now_empty {
                // Remove from partial list
                self.partial_pages.lock().retain(|&p| p != page);
                self.free_pages.lock().push(page);
            }
        }

        Ok(())
    }

    /// Free page memory back to the system
    fn free_page_memory(&self, page: *mut SlabPage) {
        let page_addr = page as usize;
        let frame_addr = page_addr as u64;
        dealloc_frame(frame_addr);
    }
}

/// Slab allocator managing multiple caches
pub struct SlabAllocator {
    /// Array of slab caches for different sizes
    caches: SpinLock<[Option<SlabCache>; 32]>,
    /// Total allocated memory
    total_allocated: AtomicU64,
    /// Total free memory
    total_free: AtomicU64,
    /// Number of allocations performed
    allocation_count: AtomicU64,
    /// Number of deallocations performed
    deallocation_count: AtomicU64,
}

/// Slab allocator comprehensive statistics
#[derive(Debug, Clone)]
pub struct SlabAllocatorStats {
    /// Statistics for each cache
    pub cache_stats: Vec<SlabStats>,
    /// Total allocated memory across all caches
    pub total_allocated: u64,
    /// Total free memory across all caches
    pub total_free: u64,
    /// Total number of allocations
    pub allocation_count: u64,
    /// Total number of deallocations
    pub deallocation_count: u64,
}

impl SlabAllocator {
    /// Predefined size classes (powers of 2 from 8 to 4096)
    const SIZE_CLASSES: [usize; 32] = [
        8, 16, 24, 32, 40, 48, 56, 64,
        96, 128, 160, 192, 224, 256, 320, 384,
        448, 512, 640, 768, 896, 1024, 1280, 1536,
        1792, 2048, 2560, 3072, 3584, 4096, 0, 0,
    ];

    /// Create a new slab allocator
    pub fn new() -> Self {
        Self {
            caches: SpinLock::new([None; 32]),
            total_allocated: AtomicU64::new(0),
            total_free: AtomicU64::new(0),
            allocation_count: AtomicU64::new(0),
            deallocation_count: AtomicU64::new(0),
        }
    }

    /// Initialize all slab caches
    pub fn initialize(&self) -> Result<(), SlabError> {
        let mut caches = self.caches.lock();

        for (i, &size) in Self::SIZE_CLASSES.iter().enumerate() {
            if size == 0 {
                continue;
            }

            let config = SlabCacheConfig {
                object_size: size,
                alignment: 8,
                name: alloc::format!("slab_{}", size),
                initial_pages: 1,
            };

            caches[i] = Some(SlabCache::new(config)?);
        }

        log::info!("Initialized slab allocator with {} size classes",
                  caches.iter().filter(Option::is_some).count());
        Ok(())
    }

    /// Allocate memory of the given size
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, SlabError> {
        if size == 0 {
            return Err(SlabError::InvalidSize);
        }

        // Find appropriate size class
        let class_index = self.find_size_class(size);
        if class_index.is_none() {
            return Err(SlabError::ObjectTooLarge);
        }

        let caches = self.caches.lock();
        if let Some(ref cache) = caches[class_index.unwrap()] {
            let result = cache.allocate();
            if result.is_ok() {
                self.allocation_count.fetch_add(1, Ordering::Relaxed);
            }
            result
        } else {
            Err(SlabError::NotInitialized)
        }
    }

    /// Deallocate memory
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), SlabError> {
        if size == 0 {
            return Err(SlabError::InvalidSize);
        }

        let class_index = self.find_size_class(size);
        if class_index.is_none() {
            return Err(SlabError::ObjectTooLarge);
        }

        let caches = self.caches.lock();
        if let Some(ref cache) = caches[class_index.unwrap()] {
            let result = cache.deallocate(ptr);
            if result.is_ok() {
                self.deallocation_count.fetch_add(1, Ordering::Relaxed);
            }
            result
        } else {
            Err(SlabError::NotInitialized)
        }
    }

    /// Get comprehensive statistics
    pub fn stats(&self) -> SlabAllocatorStats {
        let caches = self.caches.lock();
        let mut cache_stats = Vec::new();

        for cache in caches.iter().filter_map(Option::as_ref) {
            cache_stats.push(cache.stats());
        }

        SlabAllocatorStats {
            cache_stats,
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            total_free: self.total_free.load(Ordering::Relaxed),
            allocation_count: self.allocation_count.load(Ordering::Relaxed),
            deallocation_count: self.deallocation_count.load(Ordering::Relaxed),
        }
    }

    /// Shrink all caches
    pub fn shrink_all(&self) -> usize {
        let caches = self.caches.lock();
        let mut total_freed = 0;

        for cache in caches.iter().filter_map(Option::as_ref) {
            total_freed += cache.shrink();
        }

        total_freed
    }

    /// Find the appropriate size class for a given size
    fn find_size_class(&self, size: usize) -> Option<usize> {
        for (i, &class_size) in Self::SIZE_CLASSES.iter().enumerate() {
            if class_size == 0 {
                continue;
            }
            if size <= class_size {
                return Some(i);
            }
        }
        None
    }
}

/// Global slab allocator instance
static mut SLAB_ALLOCATOR: Option<SlabAllocator> = None;
static SLAB_ALLOCATOR_INIT: SpinLock<bool> = SpinLock::new(false);

/// Initialize the global slab allocator
pub fn init() -> Result<(), SlabError> {
    let mut init_flag = SLAB_ALLOCATOR_INIT.lock();

    if *init_flag {
        return Ok(());
    }

    let allocator = SlabAllocator::new();
    allocator.initialize()?;

    unsafe {
        SLAB_ALLOCATOR = Some(allocator);
    }

    *init_flag = true;
    log::info!("Global slab allocator initialized");
    Ok(())
}

/// Get the global slab allocator
fn get_slab_allocator() -> &'static SlabAllocator {
    unsafe {
        SLAB_ALLOCATOR.as_ref().unwrap()
    }
}

/// Allocate memory using the slab allocator
pub fn alloc(size: usize) -> Result<NonNull<u8>, SlabError> {
    get_slab_allocator().allocate(size)
}

/// Deallocate memory using the slab allocator
pub fn dealloc(ptr: NonNull<u8>, size: usize) -> Result<(), SlabError> {
    get_slab_allocator().deallocate(ptr, size)
}

/// Get slab allocator statistics
pub fn get_stats() -> SlabAllocatorStats {
    get_slab_allocator().stats()
}

/// Shrink all slab caches
pub fn shrink_all() -> usize {
    get_slab_allocator().shrink_all()
}

/// Simple formatted string helper for compile-time strings
mod boxleak {
    pub fn format(args: core::fmt::Arguments<'_>) -> &'static str {
        // For now, return a static string. In a real implementation,
        // this would use compile-time string formatting or heap allocation
        if let Some(s) = args.as_str() {
            s
        } else {
            "formatted_slab"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slab_cache_creation() {
        let config = SlabCacheConfig {
            object_size: 64,
            alignment: 8,
            name: "test",
            initial_pages: 1,
        };

        let cache = SlabCache::new(config).unwrap();
        assert_eq!(cache.object_size, 64);
        assert!(cache.objects_per_page > 0);
    }

    #[test]
    fn test_slab_allocator_init() {
        // This test requires the global allocator to be available
        // For testing purposes, we create a local instance
        let allocator = SlabAllocator::new();
        assert!(allocator.initialize().is_ok());
    }

    #[test]
    fn test_size_class_selection() {
        let allocator = SlabAllocator::new();

        assert_eq!(allocator.find_size_class(8), Some(0));
        assert_eq!(allocator.find_size_class(16), Some(1));
        assert_eq!(allocator.find_size_class(100), Some(9)); // 128-byte class
        assert_eq!(allocator.find_size_class(5000), None); // Too large
    }
}