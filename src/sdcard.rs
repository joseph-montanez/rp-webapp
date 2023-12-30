use core::cell::RefCell;
use embassy_rp::gpio::{Output};
use embassy_rp::peripherals::{PIN_9, SPI1};
use embassy_rp::spi::{Async, Spi};
use embedded_sdmmc::{Attributes, BlockDevice, Directory, File, Mode, SdCard, ShortFileName, TimeSource, Timestamp, Volume, VolumeIdx, VolumeManager};
use embedded_hal::blocking::delay::DelayUs;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use critical_section::with;
use embassy_usb::UsbDeviceState::Default;
use crate::http::ByteString;
use crate::sdcard::SdCardError::{FileOpenError, VolumeCloseError, VolumeError};
use lorawan::parser::AsPhyPayloadBytes;

#[derive(Clone)]
#[derive(Copy)]
pub struct FileInfo {
    pub name: [u8; 64],
    pub name_len: usize,
    pub is_dir: bool,
    pub size: u32,
    pub mtime: Timestamp,
}


pub struct MyTimeSource;

impl TimeSource for MyTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        // Return a dummy timestamp or real-time clock value
        Timestamp { year_since_1970: 0, zero_indexed_month: 0, zero_indexed_day: 0, hours: 0, minutes: 0, seconds: 0 }
    }
}

pub struct Delayer;

impl DelayUs<u8> for Delayer {
    fn delay_us(&mut self, us: u8) {
        // Implement a blocking delay here.
        // Since Rust on microcontrollers doesn't have a standard way to block for time,
        // this implementation will depend on the specifics of your platform and setup.
        // For example, you might use a simple loop, or if available, a hardware timer.
    }
}

pub enum SdCardError {
    NoError,
    VolumeError,
    VolumeOpenError,
    VolumeCloseError,
    DirectoryOpenError,
    DirectoryCloseError,
    FileOpenError,
    FileCloseError,
    FileReadError,
    FileWriteError,
    FileDeleteError,
    DirectoryReadError,
}

struct DirGuard<'a>
{
    sd_card_ops: &'a mut dyn SdCardOperations,
    dir: Directory,
}

impl<'a> Drop for DirGuard<'a>
{
    fn drop(&mut self) {
        self.sd_card_ops.close_directory(&self.dir);
    }
}

struct VolGuard<'a> {
    sd_card_ops: &'a mut dyn SdCardOperations,
    vol: Volume,
}

impl<'a> Drop for VolGuard<'a> {
    fn drop(&mut self) {
        // Call the appropriate method to close the volume
        self.sd_card_ops.close_volume(&self.vol);
    }
}

// Define a non-generic trait
trait SdCardOperations {
    fn open_volume(&mut self) -> Result<Volume, SdCardError>;
    fn open_root_directory(&mut self, volume: &Volume) -> Result<Directory, SdCardError>;
    fn close_volume(&mut self, volume: &Volume);
    fn close_directory(&mut self, directory: &Directory);
    fn open_file_in_dir(&mut self, dir: &Directory, file_path: &str, mode: Mode) -> Result<File, SdCardError>;
    fn read(&mut self, file: &File, buffer: &mut [u8]) -> Result<usize, SdCardError>;

}

// Implement this trait for a struct holding VolumeManager
pub struct SdCardManager<'a>
{
    pub(crate) volume_mgr: VolumeManager<SdCard<Spi<'static, SPI1, embassy_rp::spi::Async>, Output<'a, PIN_9>, Delayer>, MyTimeSource>,
    // Other fields...
}

impl SdCardOperations for SdCardManager<'_>
{
    fn open_volume(&mut self) -> Result<Volume, SdCardError> {
        self.volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| SdCardError::VolumeError)
    }

    fn open_root_directory(&mut self, volume: &Volume) -> Result<Directory, SdCardError> {
        self.volume_mgr.open_root_dir(*volume).map_err(|_| SdCardError::DirectoryOpenError)
    }

    fn close_volume(&mut self, volume: &Volume) {
        let _ = self.volume_mgr.close_volume(*volume);
    }

    fn close_directory(&mut self, directory: &Directory) {
        let _ = self.volume_mgr.close_dir(*directory);
    }

    fn open_file_in_dir(&mut self, dir: &Directory, file_path: &str, mode: Mode) -> Result<File, SdCardError> {
        self.volume_mgr.open_file_in_dir(*dir, "my_file.TXT", embedded_sdmmc::Mode::ReadOnly).map_err(|_| SdCardError::FileOpenError)
    }

    fn read(&mut self, file: &File, buffer: &mut [u8]) -> Result<usize, SdCardError> {
        match self.volume_mgr.file_eof(*file) {
            Ok(eof) => {
                if eof {
                    return Ok(0); // EOF reached
                }
            },
            Err(_) => {
                return Err(SdCardError::FileReadError); // Error checking EOF
            }
        }

        match self.volume_mgr.read(*file, buffer) {
            Ok(read) => Ok(read),
            Err(_) => Err(SdCardError::FileReadError), // Error reading file
        }
    }
}

pub trait ReadCallback {
    fn call(&mut self, data: &[u8]) -> Result<(), SdCardError>;
}

pub static mut CALLBACK: Option<&'static mut dyn ReadCallback> = None;

pub static SDCARD_MANAGER: CriticalSectionMutex<RefCell<Option<SdCardManager>>> = CriticalSectionMutex::new(RefCell::new(None));

#[embassy_executor::task]
pub async fn read_file_async(file_path: &'static str) {
    with(|cs| {
        if let Some(manager) = SDCARD_MANAGER.borrow(cs).borrow_mut().as_mut() {
            // Open volume
            let volume = match manager.open_volume() {
                Ok(vol) => vol,
                Err(_) => return, // handle error
            };

            // Use the volume to open the root directory
            let root_dir = match manager.open_root_directory(&volume) {
                Ok(dir) => dir,
                Err(_) => {
                    // Close the volume before returning
                    manager.close_volume(&volume);
                    return; // handle error
                }
            };

            // Open file within the root directory
            let file = match manager.open_file_in_dir(&root_dir, file_path, Mode::ReadOnly) {
                Ok(file) => file,
                Err(_) => {
                    // Close the directory and volume before returning
                    manager.close_directory(&root_dir);
                    manager.close_volume(&volume);
                    return; // handle error
                }
            };

            // Read file in chunks
            let mut buffer = [0u8; 32]; // Define your buffer size
            loop {
                match manager.read(&file, &mut buffer) {
                    Ok(read) => {
                        if read == 0 { break; }
                        unsafe {
                            if let Some(callback) = CALLBACK.as_mut() {
                                // Use `callback.call(data)` to trigger the callback
                                let _ = callback.call(&buffer[..read]);
                            }
                        }
                    },
                    Err(_) => {
                        // Handle read error
                        break;
                    },
                };
            }

            // Close the file, directory, and volume
            // (Assuming there are methods to close the file and directory)
            let _ = manager.volume_mgr.close_file(file);
            manager.close_directory(&root_dir);
            manager.close_volume(&volume);
        }
    });
}

pub fn read_file<'a, const N: usize>(
    mut volume_mgr: &mut VolumeManager<SdCard<Spi<SPI1, Async>, Output<PIN_9>, Delayer>, MyTimeSource>,
    file_path: &'static str,
    out: &mut ByteString<N>
) -> Result<usize, SdCardError> {
    let volume = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)).map_err(|_| SdCardError::VolumeError)?;
    let root_dir = volume_mgr.open_root_dir(volume).map_err(|_| SdCardError::DirectoryOpenError)?;
    let my_file = volume_mgr.open_file_in_dir(root_dir, file_path, embedded_sdmmc::Mode::ReadOnly).map_err(|_| SdCardError::FileOpenError)?;

    let mut bytes_read = 0;

    loop {
        let eof = volume_mgr.file_eof(my_file).map_err(|_| SdCardError::FileReadError)?;
        if eof {
            break;
        }

        let mut buffer = [0u8; 32];
        let read = volume_mgr.read(my_file, &mut buffer).map_err(|_| SdCardError::FileReadError)?;
        out.append(&buffer[..read]);
        bytes_read += read;
    }

    // Close the file and handle potential errors
    volume_mgr.close_file(my_file).map_err(|_| SdCardError::FileCloseError)?;
    volume_mgr.close_dir(root_dir).map_err(|_| SdCardError::DirectoryCloseError)?;
    volume_mgr.close_volume(volume).map_err(|_| SdCardError::VolumeCloseError)?;

    Ok(bytes_read)
}

pub fn write_file<'a, const N: usize>(
    mut volume_mgr: &mut VolumeManager<SdCard<Spi<SPI1, Async>, Output<PIN_9>, Delayer>, MyTimeSource>,
    file_path: &'static str,
    data: &[u8]
) -> Result<(), SdCardError> {
    let volume = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)).map_err(|_| SdCardError::VolumeError)?;
    let root_dir = volume_mgr.open_root_dir(volume).map_err(|_| SdCardError::DirectoryOpenError)?;

    // Open the file in write mode, create it if it doesn't exist
    let my_file = volume_mgr.open_file_in_dir(root_dir, file_path, embedded_sdmmc::Mode::ReadWriteCreateOrTruncate).map_err(|_| SdCardError::FileOpenError)?;

    // Write data to the file
    volume_mgr.write(my_file, data).map_err(|_| SdCardError::FileWriteError)?;

    // Close the file and handle potential errors
    volume_mgr.close_file(my_file).map_err(|_| SdCardError::FileCloseError)?;
    volume_mgr.close_dir(root_dir).map_err(|_| SdCardError::DirectoryCloseError)?;
    volume_mgr.close_volume(volume).map_err(|_| SdCardError::VolumeCloseError)?;

    Ok(())
}

pub fn check_file_exists(
    mut volume_mgr: &mut VolumeManager<SdCard<Spi<SPI1, Async>, Output<PIN_9>, Delayer>, MyTimeSource>,
    file_path: &'static str
) -> bool {
    let volume = match volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)) {
        Ok(vol) => vol,
        Err(_) => return false,
    };

    let root_dir = match volume_mgr.open_root_dir(volume) {
        Ok(dir) => dir,
        Err(_) => return false,
    };

    let file_open_result = volume_mgr.open_file_in_dir(root_dir, file_path, embedded_sdmmc::Mode::ReadOnly);

    // Close the directory and volume after the check
    let _ = volume_mgr.close_dir(root_dir);
    let _ = volume_mgr.close_volume(volume);

    // Check if file open was successful
    file_open_result.is_ok()
}

pub fn delete_file(
    mut volume_mgr: &mut VolumeManager<SdCard<Spi<SPI1, Async>, Output<PIN_9>, Delayer>, MyTimeSource>,
    file_path: &str,
) -> Result<(), SdCardError> {
    // Open the volume
    let volume = volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| SdCardError::VolumeError)?;

    // Open the root directory
    let root_dir = volume_mgr.open_root_dir(volume).map_err(|_| SdCardError::DirectoryOpenError)?;

    // Delete the file from the directory
    volume_mgr.delete_file_in_dir(root_dir, file_path).map_err(|_| SdCardError::FileDeleteError)?;

    // Close the directory
    volume_mgr.close_dir(root_dir).map_err(|_| SdCardError::DirectoryCloseError)?;

    Ok(())
}

pub fn list_directory(
    volume_mgr: &mut VolumeManager<SdCard<Spi<SPI1, Async>, Output<PIN_9>, Delayer>, MyTimeSource>,
    dir_path: &str,
) -> Result<([FileInfo; 64], usize), SdCardError> {
    // Open the volume
    let volume = volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| SdCardError::VolumeError)?;

    // Open the specified directory
    let root_dir = volume_mgr.open_root_dir(volume).map_err(|_| SdCardError::DirectoryOpenError)?;
    let directory = if dir_path == "/" {
        root_dir
    } else {
        volume_mgr.open_dir(root_dir, ShortFileName::create_from_str(dir_path).map_err(|_| SdCardError::DirectoryOpenError)?)
            .map_err(|_| SdCardError::DirectoryOpenError)?
    };

    let mut files = [FileInfo {
        name: [0; 64],
        name_len: 0,
        is_dir: false,
        size: 0,
        mtime: Timestamp::from_fat(0, 0),
    }; 64];
    let mut file_count = 0;

    volume_mgr.iterate_dir(directory, |entry| {
        if file_count < 64 {
            let mut name = [0u8; 64];
            let basename = entry.name.base_name();
            let extension = entry.name.extension();

            let basename_length = basename.len();
            let extension_length = extension.len();

            if basename_length + extension_length + (if extension_length > 0 { 1 } else { 0 }) <= 64 {
                // Copy the base name
                name[..basename_length].copy_from_slice(basename);

                // Copy the extension, with a dot if the extension exists
                if extension_length > 0 {
                    name[basename_length] = b'.'; // Adding a dot before the extension
                    name[basename_length + 1..basename_length + 1 + extension_length].copy_from_slice(extension);
                }

                // Calculate the total length of the new name
                let name_len = basename_length + (if extension_length > 0 { 1 + extension_length } else { 0 });
                files[file_count].name[..name_len].copy_from_slice(&name[..name_len]);
                files[file_count].name_len = name_len;
            } else {}

            files[file_count].is_dir = entry.attributes.is_directory();
            files[file_count].size = entry.size;
            files[file_count].mtime = entry.mtime;
            file_count += 1;
        }
    }).map_err(|_| SdCardError::DirectoryReadError)?;

    // Close the directory and volume
    volume_mgr.close_dir(directory).map_err(|_| SdCardError::DirectoryCloseError)?;
    volume_mgr.close_volume(volume).map_err(|_| SdCardError::VolumeCloseError)?;

    Ok((files, file_count))
}

pub fn append_to_file(
    mut volume_mgr: &mut VolumeManager<SdCard<Spi<SPI1, Async>, Output<PIN_9>, Delayer>, MyTimeSource>,
    file_path: &str,
    data: &[u8],
) -> Result<(), SdCardError> {
    // Open the volume
    let volume = volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| SdCardError::VolumeError)?;

    // Open the root directory
    let root_dir = volume_mgr.open_root_dir(volume).map_err(|_| SdCardError::DirectoryOpenError)?;

    // Open the file in ReadWriteAppend mode
    let file = volume_mgr.open_file_in_dir(root_dir, file_path, Mode::ReadWriteAppend)
        .map_err(|_| SdCardError::FileOpenError)?;

    // Write data to the file
    volume_mgr.write(file, data).map_err(|_| SdCardError::FileWriteError)?;

    // Close the file
    volume_mgr.close_file(file).map_err(|_| SdCardError::FileCloseError)?;

    // Close the directory
    volume_mgr.close_dir(root_dir).map_err(|_| SdCardError::DirectoryCloseError)?;

    Ok(())
}
