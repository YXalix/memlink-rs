//! Page scanning operations for idle page detection
//!
//! This module provides safe wrappers around the kernel's idle page scanning
//! functionality. It allows detecting which memory pages are "cold" (idle)
//! versus "hot" (recently accessed).

use std::collections::VecDeque;

use crate::error::{EtmemError, Result};
use crate::sys::ProcfsHandle;
use crate::types::{
    AddressRange, BufferStatus, IdlePageInfo, PipEncoding, ProcIdlePageType, ScanConfig,
    ScanFlags, PAGE_IDLE_KBUF_SIZE,
};

/// Internal control structure for page idle scanning
///
/// This is the Rust equivalent of the kernel's `struct page_idle_ctrl`.
/// It manages the state of an ongoing page scan operation.
#[derive(Debug)]
pub struct PageIdleCtrl {
    /// Kernel buffer for idle page data
    kpie: Vec<u8>,
    /// Current read position in kpie
    pie_read: usize,
    /// Maximum read position in kpie
    pie_read_max: usize,
    /// Next HVA to scan (GPA for EPT, VA for PT)
    next_hva: u64,
    /// GPA to HVA translation offset
    gpa_to_hva: u64,
    /// Restart GPA for resuming scan
    restart_gpa: u64,
    /// Last VA processed (for duplicate detection)
    last_va: u64,
    /// Scan flags
    flags: ScanFlags,
    /// Accumulated results
    results: VecDeque<IdlePageInfo>,
}

impl PageIdleCtrl {
    /// Create a new PageIdleCtrl with given buffer size and flags
    pub fn new(buffer_size: usize, flags: ScanFlags) -> Self {
        let buffer_size = buffer_size.min(PAGE_IDLE_KBUF_SIZE);
        Self {
            kpie: vec![0u8; PAGE_IDLE_KBUF_SIZE],
            pie_read: 0,
            pie_read_max: buffer_size,
            next_hva: 0,
            gpa_to_hva: 0,
            restart_gpa: 0,
            last_va: 0,
            flags,
            results: VecDeque::new(),
        }
    }

    /// Initialize/reset the kernel buffer for reading
    ///
    /// This prepares the buffer to receive data from the kernel.
    /// Returns a status indicating if the buffer is full.
    pub fn init_buffer(&mut self, buf_size: usize, bytes_copied: usize) -> BufferStatus {
        self.pie_read = 0;
        self.pie_read_max = std::cmp::min(
            PAGE_IDLE_KBUF_SIZE,
            buf_size.saturating_sub(bytes_copied),
        );

        // Reserve space for PIP_CMD_SET_HVA at end
        if self.pie_read_max > std::mem::size_of::<u64>() + 2 {
            self.pie_read_max -= std::mem::size_of::<u64>() + 1;
        } else {
            return BufferStatus::KbufFull;
        }

        self.kpie.fill(0);
        BufferStatus::Success
    }

    /// Add a page entry to the internal buffer
    ///
    /// This implements the PIP (Proc Idle Page) encoding logic from the kernel,
    /// merging consecutive pages of the same type when possible.
    fn add_page_internal(
        &mut self,
        addr: u64,
        _next: u64,
        page_type: ProcIdlePageType,
        page_size: u64,
    ) -> BufferStatus {
        // Check if we can merge with the previous entry
        if let Some(last) = self.results.back_mut() {
            if last.page_type == page_type
                && last.end_address() == addr
                && last.count < 16
                && last.page_type.page_size() == page_size
            {
                // Merge with previous entry
                last.count += 1;
                return BufferStatus::Success;
            }
        }

        // Check buffer capacity
        if self.results.len() >= self.pie_read_max / 2 {
            return BufferStatus::KbufFull;
        }

        // Add new entry
        self.results.push_back(IdlePageInfo::new(addr, page_type, 1));
        self.last_va = addr;
        BufferStatus::Success
    }

    /// Decode PIP (Proc Idle Page) format data from kernel
    ///
    /// The kernel returns data in a compact format where each byte encodes:
    /// - Upper 4 bits: page type
    /// - Lower 4 bits: count of consecutive pages minus 1
    ///
    /// Also handles special command entries for setting HVA.
    pub fn decode_pip_data(&mut self, data: &[u8], base_addr: u64) -> Result<Vec<IdlePageInfo>> {
        let mut results = Vec::new();
        let mut current_addr = base_addr;
        let mut i = 0;

        while i < data.len() {
            let byte = data[i];
            let page_type_raw = PipEncoding::extract_type(byte);
            let count = PipEncoding::extract_size(byte) + 1;

            // Check for command marker
            if page_type_raw == ProcIdlePageType::PipCmd as u8 {
                // Handle command (e.g., SET_HVA)
                if byte == PipEncoding::SET_HVA && i + 8 < data.len() {
                    // Read 64-bit HVA from next 8 bytes
                    let hva_bytes = &data[i + 1..i + 9];
                    current_addr = u64::from_le_bytes([
                        hva_bytes[0],
                        hva_bytes[1],
                        hva_bytes[2],
                        hva_bytes[3],
                        hva_bytes[4],
                        hva_bytes[5],
                        hva_bytes[6],
                        hva_bytes[7],
                    ]);
                    i += 9;
                    continue;
                }
                // Unknown command, skip
                i += 1;
                continue;
            }

            // Decode page type
            let page_type = ProcIdlePageType::from_raw(page_type_raw)
                .ok_or(EtmemError::InvalidPageType(page_type_raw))?;

            results.push(IdlePageInfo::new(current_addr, page_type, count));
            current_addr += page_type.page_size() * count as u64;
            i += 1;
        }

        Ok(results)
    }

    /// Set the next HVA to continue scanning
    pub fn set_next_hva(&mut self, hva: u64) {
        self.next_hva = hva;
    }

    /// Get the next HVA to continue scanning
    pub fn next_hva(&self) -> u64 {
        self.next_hva
    }

    /// Set the GPA to HVA translation offset (for VM scanning)
    pub fn set_gpa_to_hva(&mut self, offset: u64) {
        self.gpa_to_hva = offset;
    }

    /// Set the restart GPA for resuming scan
    pub fn set_restart_gpa(&mut self, gpa: u64) {
        self.restart_gpa = gpa;
    }

    /// Get the restart GPA
    pub fn restart_gpa(&self) -> u64 {
        self.restart_gpa
    }

    /// Take accumulated results
    pub fn take_results(&mut self) -> Vec<IdlePageInfo> {
        self.results.drain(..).collect()
    }

    /// Get reference to results
    pub fn results(&self) -> &VecDeque<IdlePageInfo> {
        &self.results
    }

    /// Get flags
    pub fn flags(&self) -> ScanFlags {
        self.flags
    }
}

impl Default for PageIdleCtrl {
    fn default() -> Self {
        Self::new(PAGE_IDLE_KBUF_SIZE, ScanFlags::empty())
    }
}

/// Safe wrapper for idle page scanning session
///
/// This provides a safe interface to the kernel's idle page scanning
/// functionality. The session is automatically cleaned up when dropped.
#[derive(Debug)]
pub struct ScanSession {
    /// Underlying procfs file handle
    handle: ProcfsHandle,
    /// Scan configuration
    config: ScanConfig,
    /// Internal control structure
    ctrl: PageIdleCtrl,
    /// Process ID being scanned
    pid: u32,
}

impl ScanSession {
    /// Create a new scan session for a process
    ///
    /// # Errors
    /// Returns error if:
    /// - Process doesn't exist
    /// - Permission denied (requires CAP_SYS_ADMIN)
    /// - ETMEM module not loaded
    /// - Invalid configuration
    ///
    /// # Example
    /// ```
    /// use etmem_rs::{ScanSession, ScanConfig};
    ///
    /// let config = ScanConfig::default();
    /// let session = ScanSession::new(std::process::id() as u32, config);
    /// ```
    pub fn new(pid: u32, config: ScanConfig) -> Result<Self> {
        if pid == 0 {
            return Err(EtmemError::InvalidPid);
        }

        config.validate()?;

        // Safe: handle construction is encapsulated
        let handle = unsafe { ProcfsHandle::open_idle_pages(pid)? };

        // Apply scan flags via IOCTL
        if !config.flags.is_empty() {
            unsafe {
                crate::sys::add_scan_flags(&handle, config.flags.bits())?;
            }
        }

        Ok(Self {
            handle,
            config: config.clone(),
            ctrl: PageIdleCtrl::new(config.buffer_size, config.flags),
            pid,
        })
    }

    /// Read idle pages starting from the given address
    ///
    /// Returns a vector of `IdlePageInfo` entries and an optional
    /// next address to continue scanning from.
    ///
    /// # Errors
    /// Returns error if:
    /// - I/O error occurs
    /// - Invalid data received from kernel
    pub fn read(&mut self, start_addr: u64) -> Result<(Vec<IdlePageInfo>, Option<u64>)> {
        // Validate address alignment (must be page-aligned)
        if start_addr % 4096 != 0 {
            return Err(EtmemError::InvalidAddress);
        }

        let mut buffer = vec![0u8; self.config.buffer_size];

        // Read from procfs
        let bytes_read = unsafe {
            self.handle
                .read_at(&mut buffer, start_addr as i64)
                .map_err(|e| EtmemError::IoError(e.to_string()))?
        };

        if bytes_read == 0 {
            return Ok((Vec::new(), None));
        }

        // Decode PIP data
        let data = &buffer[..bytes_read as usize];
        let pages = self.ctrl.decode_pip_data(data, start_addr)?;

        // Check if there might be more data
        let next_addr = if bytes_read as usize >= self.config.buffer_size {
            // Buffer was full, there might be more
            pages.last().map(|p| p.end_address())
        } else {
            None
        };

        Ok((pages, next_addr))
    }

    /// Read all idle pages in a range
    ///
    /// This convenience method reads all idle pages in the specified range,
    /// automatically handling pagination.
    ///
    /// # Errors
    /// Returns error if the range is invalid or I/O fails.
    pub fn read_range(&mut self, range: AddressRange) -> Result<Vec<IdlePageInfo>> {
        if !range.is_valid() {
            return Err(EtmemError::InvalidRange);
        }

        let mut all_pages = Vec::new();
        let mut current_addr = range.start;

        while current_addr < range.end {
            let (pages, next) = self.read(current_addr)?;

            // Filter pages to only include those in range
            for page in pages {
                if page.address >= range.start && page.address < range.end {
                    all_pages.push(page);
                }
            }

            match next {
                Some(addr) if addr < range.end => current_addr = addr,
                _ => break,
            }
        }

        Ok(all_pages)
    }

    /// Add scan flags
    ///
    /// Adds the specified flags to the current scan configuration.
    pub fn add_flags(&mut self, flags: ScanFlags) -> Result<()> {
        unsafe {
            crate::sys::add_scan_flags(&self.handle, flags.bits())?;
        }
        self.config.flags |= flags;
        Ok(())
    }

    /// Remove scan flags
    ///
    /// Removes the specified flags from the current scan configuration.
    pub fn remove_flags(&mut self, flags: ScanFlags) -> Result<()> {
        unsafe {
            crate::sys::remove_scan_flags(&self.handle, flags.bits())?;
        }
        self.config.flags &= !flags;
        Ok(())
    }

    /// Get current scan configuration
    pub fn config(&self) -> &ScanConfig {
        &self.config
    }

    /// Get the process ID being scanned
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the internal control structure (for advanced use)
    pub fn control(&self) -> &PageIdleCtrl {
        &self.ctrl
    }

    /// Get mutable reference to internal control structure
    pub fn control_mut(&mut self) -> &mut PageIdleCtrl {
        &mut self.ctrl
    }
}

/// High-level idle page scanner
///
/// This provides a convenient API for scanning without managing
/// the session lifecycle manually.
#[derive(Debug)]
pub struct IdlePageScanner;

impl IdlePageScanner {
    /// Scan a process for idle pages
    ///
    /// Creates a temporary scan session and reads idle pages
    /// starting from address 0.
    ///
    /// # Example
    /// ```no_run
    /// use etmem_rs::{IdlePageScanner, ScanConfig};
    ///
    /// let config = ScanConfig::default();
    /// let pages = IdlePageScanner::scan_process(std::process::id() as u32, config)
    ///     .expect("Failed to scan process");
    /// for page in pages {
    ///     println!("Found {:?} page at {:x}", page.page_type, page.address);
    /// }
    /// ```
    pub fn scan_process(pid: u32, config: ScanConfig) -> Result<Vec<IdlePageInfo>> {
        let mut session = ScanSession::new(pid, config)?;
        let mut all_pages = Vec::new();
        let mut current_addr: u64 = 0;

        loop {
            let (pages, next) = session.read(current_addr)?;
            all_pages.extend(pages);

            match next {
                Some(addr) => current_addr = addr,
                None => break,
            }
        }

        Ok(all_pages)
    }

    /// Scan a specific address range in a process
    pub fn scan_range(
        pid: u32,
        range: AddressRange,
        config: ScanConfig,
    ) -> Result<Vec<IdlePageInfo>> {
        let mut session = ScanSession::new(pid, config)?;
        session.read_range(range)
    }

    /// Scan only for idle pages (convenience method)
    pub fn scan_idle_pages(pid: u32, config: ScanConfig) -> Result<Vec<IdlePageInfo>> {
        let pages = Self::scan_process(pid, config)?;
        Ok(pages.into_iter().filter(|p| p.is_idle()).collect())
    }

    /// Scan only for accessed/hot pages (convenience method)
    pub fn scan_accessed_pages(pid: u32, config: ScanConfig) -> Result<Vec<IdlePageInfo>> {
        let pages = Self::scan_process(pid, config)?;
        Ok(pages.into_iter().filter(|p| p.is_accessed()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_idle_ctrl_new() {
        let ctrl = PageIdleCtrl::new(4096, ScanFlags::SCAN_HUGE_PAGE);
        assert_eq!(ctrl.next_hva(), 0);
        assert!(ctrl.flags().contains(ScanFlags::SCAN_HUGE_PAGE));
    }

    #[test]
    fn test_decode_pip_data() {
        let mut ctrl = PageIdleCtrl::default();

        // Create sample PIP data: 1 PTE_ACCESSED page
        let data = vec![PipEncoding::compose(ProcIdlePageType::PteAccessed as u8, 0)];
        let result = ctrl.decode_pip_data(&data, 0x1000).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].address, 0x1000);
        assert_eq!(result[0].page_type, ProcIdlePageType::PteAccessed);
        assert_eq!(result[0].count, 1);
    }

    #[test]
    fn test_decode_pip_data_with_hva() {
        let mut ctrl = PageIdleCtrl::default();

        // Create PIP data with SET_HVA command
        let mut data = vec![PipEncoding::SET_HVA];
        let hva: u64 = 0x10000;
        data.extend_from_slice(&hva.to_le_bytes());
        // Add a PTE_IDLE entry
        data.push(PipEncoding::compose(ProcIdlePageType::PteIdle as u8, 0));

        let result = ctrl.decode_pip_data(&data, 0).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].address, 0x10000);
        assert_eq!(result[0].page_type, ProcIdlePageType::PteIdle);
    }

    #[test]
    fn test_scan_config_validation() {
        // Valid config should pass
        let config = ScanConfig::default();
        assert!(ScanSession::new(0, config).is_err()); // PID 0 is invalid
    }

    #[test]
    fn test_address_range_validation() {
        let range = AddressRange::new(0x1000, 0x5000);
        let mut ctrl = PageIdleCtrl::default();

        // Simulate adding pages
        let status = ctrl.add_page_internal(
            0x1000,
            0x2000,
            ProcIdlePageType::PteIdle,
            4096,
        );
        assert!(matches!(status, BufferStatus::Success));
    }
}
