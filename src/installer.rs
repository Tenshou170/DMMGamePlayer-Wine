use std::{
    env,
    fs,
    path::{Path, PathBuf},
};
use crate::registry;
use crate::utils::is_dmm_running;
use pelite::resources::version_info::Language;

pub struct Installer {
    pub install_dir: PathBuf,
    pub version: String,
}

impl Default for Installer {
    fn default() -> Self {
        let default_path = PathBuf::from(r"C:\Program Files\DMMGamePlayer");
        Self {
            install_dir: default_path,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl Installer {
    pub fn new<P: AsRef<Path>>(target_dir: Option<P>) -> Self {
        let mut inst = Self::default();
        if let Some(dir) = target_dir {
            inst.install_dir = dir.as_ref().to_path_buf();
        }
        inst
    }

    pub fn get_installed_version(&self) -> Option<String> {
        let exe_path = self.install_dir.join("DMMGamePlayer.exe");
        if exe_path.exists() {
            if let Ok(map) = pelite::FileMap::open(&exe_path) {
                if let Some(version_info) = crate::utils::read_pe_version_info(map.as_ref()) {
                    let lang_neutral = Language {
                        lang_id: 0x0000,
                        charset_id: 0x04b0,
                    };
                    let lang_english = Language {
                        lang_id: 0x0409,
                        charset_id: 0x04b0,
                    };
                    if let Some(v) = version_info.value(lang_neutral, "ProductVersion")
                        .or_else(|| version_info.value(lang_english, "ProductVersion"))
                        .or_else(|| version_info.value(lang_neutral, "FileVersion"))
                        .or_else(|| version_info.value(lang_english, "FileVersion"))
                    {
                        return Some(v.to_string());
                    }
                }
            }
        }

        // Fallback: check registry DisplayVersion
        registry::get_installed_dmm_version()
    }

    pub fn locate_source_payload(&self) -> Option<PathBuf> {
        let current_exe = env::current_exe().ok()?;
        let exe_dir = current_exe.parent()?;

        let candidates = [
            exe_dir.join("DMMGamePlayer"),
            exe_dir.join("app"),
            exe_dir.join("payload"),
            exe_dir.to_path_buf(),
        ];

        candidates.into_iter().find(|candidate| candidate.join("DMMGamePlayer.exe").exists())
    }

    /// Calculates the size of the PE image on disk by reading the section headers.
    fn calculate_pe_size(file: &mut fs::File) -> std::io::Result<u64> {
        use std::io::{Read, Seek, SeekFrom};

        let mut dos_header = [0u8; 64];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut dos_header)?;

        if dos_header[0] != b'M' || dos_header[1] != b'Z' {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not a valid PE executable (invalid DOS signature)",
            ));
        }

        let pe_offset = u32::from_le_bytes(dos_header[60..64].try_into().unwrap()) as u64;
        file.seek(SeekFrom::Start(pe_offset))?;

        let mut pe_signature = [0u8; 4];
        file.read_exact(&mut pe_signature)?;
        if &pe_signature != b"PE\0\0" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not a valid PE executable (invalid PE signature)",
            ));
        }

        let mut coff_header = [0u8; 20];
        file.read_exact(&mut coff_header)?;
        let num_sections = u16::from_le_bytes(coff_header[2..4].try_into().unwrap());
        let opt_hdr_size = u16::from_le_bytes(coff_header[16..18].try_into().unwrap());

        let sec_table_offset = pe_offset + 4 + 20 + opt_hdr_size as u64;
        file.seek(SeekFrom::Start(sec_table_offset))?;

        let mut max_end = 0u64;
        for _ in 0..num_sections {
            let mut sec = [0u8; 40];
            file.read_exact(&mut sec)?;
            let raw_size = u32::from_le_bytes(sec[16..20].try_into().unwrap()) as u64;
            let raw_offset = u32::from_le_bytes(sec[20..24].try_into().unwrap()) as u64;
            let end = raw_offset + raw_size;
            if end > max_end {
                max_end = end;
            }
        }

        Ok(max_end)
    }

    /// Extracts an embedded 7z archive attached to the installer executable as an overlay.
    pub fn extract_embedded_payload(&self) -> Result<bool, String> {
        use std::io::{Read, Seek, SeekFrom};

        let current_exe = env::current_exe().map_err(|e| e.to_string())?;
        let mut file = fs::File::open(&current_exe).map_err(|e| e.to_string())?;
        let total_size = file.metadata().map_err(|e| e.to_string())?.len();

        let pe_size = Self::calculate_pe_size(&mut file).map_err(|e| e.to_string())?;
        if total_size <= pe_size {
            return Ok(false);
        }

        // Overlay exists! Check for 7z signature (37 7A BC AF 27 1C)
        let overlay_size = total_size - pe_size;
        file.seek(SeekFrom::Start(pe_size)).map_err(|e| e.to_string())?;

        let mut magic = [0u8; 6];
        file.read_exact(&mut magic).map_err(|e| e.to_string())?;
        if magic != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
            return Ok(false);
        }

        // Extract using sevenz_rust from the overlay slice
        file.seek(SeekFrom::Start(pe_size)).map_err(|e| e.to_string())?;
        let sub_reader = SubSliceReader::new(file, pe_size, overlay_size);

        fs::create_dir_all(&self.install_dir)
            .map_err(|e| format!("Failed to create install directory: {}", e))?;

        sevenz_rust2::decompress(sub_reader, &self.install_dir)
            .map_err(|e| format!("Failed to extract embedded payload archive: {:?}", e))?;

        Ok(true)
    }

    pub fn copy_recursive<U: AsRef<Path>, V: AsRef<Path>>(
        from: U,
        to: V,
    ) -> std::io::Result<()> {
        let from = from.as_ref();
        let to = to.as_ref();
        fs::create_dir_all(to)?;

        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let dest_path = to.join(entry.file_name());

            if file_type.is_dir() {
                Self::copy_recursive(entry.path(), dest_path)?;
            } else {
                fs::copy(entry.path(), dest_path)?;
            }
        }
        Ok(())
    }

    pub fn create_sdk_log_dirs() {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            let log_dir = PathBuf::from(userprofile).join(r".DMMGAMEPLAYERSDK\log");
            let _ = fs::create_dir_all(log_dir);
        }
    }

    pub fn install(&self) -> Result<(), String> {
        if is_dmm_running() {
            return Err("DMM Game Player is currently running. Please close it first.".to_string());
        }

        // 1. First, check if an embedded payload exists in the executable's overlay
        let extracted_embedded = self.extract_embedded_payload()?;

        // 2. If not embedded, look for side-by-side payload directory
        if !extracted_embedded {
            let source = self.locate_source_payload().ok_or_else(|| {
                "Could not locate DMMGamePlayer payload (neither embedded nor side-by-side)."
                    .to_string()
            })?;

            if source != self.install_dir {
                Self::copy_recursive(&source, &self.install_dir)
                    .map_err(|e| format!("Failed to copy application files: {}", e))?;
            }
        }

        // 1b. Copy installer executable as uninstaller
        if let Ok(current_exe) = env::current_exe() {
            let uninstaller_dest = self.install_dir.join("Uninstall DMMGamePlayer.exe");
            if current_exe != uninstaller_dest {
                let _ = fs::copy(&current_exe, &uninstaller_dest);
            }
        }

        // 2. Setup SDK log directory
        Self::create_sdk_log_dirs();

        // 3. Write registry settings
        registry::apply_dmm_registry_settings(&self.install_dir, &self.version)
            .map_err(|e| format!("Failed to apply registry configuration: {}", e))?;

        Ok(())
    }

    pub fn repair_registry(&self) -> Result<(), String> {
        Self::create_sdk_log_dirs();
        registry::apply_dmm_registry_settings(&self.install_dir, &self.version)
            .map_err(|e| format!("Failed to repair registry configuration: {}", e))?;
        Ok(())
    }

    pub fn uninstall(&self) -> Result<(), String> {
        if is_dmm_running() {
            return Err("DMM Game Player is currently running. Please close it first.".to_string());
        }

        // 1. Remove registry settings
        registry::remove_dmm_registry_settings();

        // 2. Delete installation directory
        if self.install_dir.exists() {
            let _ = fs::remove_dir_all(&self.install_dir);
        }

        Ok(())
    }
}

/// A reader and seeker that wraps a portion of a file from [start_offset] to [start_offset + length].
pub struct SubSliceReader<R> {
    inner: R,
    start_offset: u64,
    length: u64,
    current_pos: u64,
}

impl<R: std::io::Read + std::io::Seek> SubSliceReader<R> {
    pub fn new(mut inner: R, start_offset: u64, length: u64) -> Self {
        let _ = inner.seek(std::io::SeekFrom::Start(start_offset));
        Self {
            inner,
            start_offset,
            length,
            current_pos: 0,
        }
    }
}

impl<R: std::io::Read + std::io::Seek> std::io::Read for SubSliceReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.current_pos >= self.length {
            return Ok(0);
        }
        let remaining = (self.length - self.current_pos) as usize;
        let to_read = buf.len().min(remaining);
        let n = self.inner.read(&mut buf[..to_read])?;
        self.current_pos += n as u64;
        Ok(n)
    }
}

impl<R: std::io::Read + std::io::Seek> std::io::Seek for SubSliceReader<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            std::io::SeekFrom::Start(offset) => offset as i64,
            std::io::SeekFrom::Current(offset) => self.current_pos as i64 + offset,
            std::io::SeekFrom::End(offset) => self.length as i64 + offset,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid seek to a negative position",
            ));
        }

        let new_pos = new_pos as u64;
        let actual_file_pos = self.start_offset + new_pos;
        self.inner.seek(std::io::SeekFrom::Start(actual_file_pos))?;
        self.current_pos = new_pos;
        Ok(self.current_pos)
    }
}

