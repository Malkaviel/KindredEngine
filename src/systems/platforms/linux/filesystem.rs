

use std::fs;
use std::path::{Path, PathBuf};
use std::fmt;
use std::io;

use std::env;

use remove_dir_all;

use core::engine_support_systems::system_management::systems::filesystems::{VFilesystem, VMetadata, VFile, OpenOptions};
use core::engine_support_systems::error_handling::error::{GameResult, GameError};
use core::engine_support_systems::system_management::System;
use core::engine_support_systems::system_management::SystemType;
use core::engine_support_systems::system_management::PlatformType;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync;

pub struct Metadata(fs::Metadata);
impl VMetadata for Metadata {
    fn is_dir(&self) -> bool {
        self.0.is_dir()
    }
    fn is_file(&self) -> bool {
        self.0.is_file()
    }
    fn len(&self) -> u64 {
        self.0.len()
    }
    fn is_read_only(&self) -> bool {
        self.0.permissions().readonly()
    }
}



#[derive(Debug)]
pub struct Filesystem {
    root: PathBuf,
    //TODO: Should the filesystem contain 'conventional paths' ? (resource directory, log directory...).
}

impl System for Filesystem {
    fn system_type(&self) -> SystemType {
        SystemType::Filesystem
    }

    fn platform(&self) -> PlatformType {
        PlatformType::Linux
    }

    fn shut_down(&self) -> GameResult<()> {
        unimplemented!();
    }
}

impl Filesystem {

    //create the filesystem and the root directory (the current directory).
    //The working directory is changed to the root directory of a unix filesystem.
    pub fn new() -> GameResult<Filesystem> {
        match env::current_dir() {
            Ok(path) => {
                Ok(Filesystem {
                    root:  path.clone(),
                })
            },
            Err(error) => Err(GameError::IOError(format!("Could not create the filesystem !"), error))
        }
    }

    //Used to check the path given by the user.
    fn get_absolute(&self, path: &Path) -> GameResult<PathBuf> {
        let mut root_path = self.root_directory();
        root_path.push(path);
        Ok(root_path)
    }
}

impl VFilesystem for Filesystem {

    fn root_directory(&self) -> PathBuf {
        self.root.clone()
    }

    fn open_with_options(&self, path: &Path, open_options: &OpenOptions) -> GameResult<Box<VFile>> {
        let absolute_path = self.get_absolute(path)?;

        open_options
            .to_fs_openoptions()
            .open(absolute_path.clone().as_path())
            .map(|file| Box::new(file) as Box<VFile>).
            map_err(GameError::from)
    }

    fn mkdir(&self, path: &Path) -> GameResult<()> {
        let absolute_path = self.get_absolute(path)?;
        fs::DirBuilder::new().recursive(true).create(absolute_path.as_path()).map_err(GameError::from)
    }

    fn rm(&self, path: &Path) -> GameResult<()> {
        let absolute_path = self.get_absolute(path)?;
        if absolute_path.is_dir() {
            fs::remove_dir(path).map_err(GameError::from)
        } else {
            fs::remove_file(path).map_err(GameError::from)
        }
    }

    fn rmrf(&self, path: &Path) -> GameResult<()> {
        let absolute_path = self.get_absolute(path)?;
        if absolute_path.is_dir() {
            match remove_dir_all::remove_dir_all(absolute_path.as_path()) {
                Ok(()) => Ok(()),
                Err(e) => Err(GameError::IOError(format!("Error while deleting the directory ({})", absolute_path.display()), e)),
            }
        } else {
            Err(GameError::FileSystemError(format!("({}) is not a directory !, use rm instead if you want to delete a file.", absolute_path.display())))
        }
    }

    fn exists(&self, path: &Path) -> bool {
        match self.get_absolute(path) {
            Ok(p) => p.exists(),
            _ => false,
        }
    }

    fn metadata(&self, path: &Path) -> GameResult<Box<VMetadata>> {
        let absolute_path = self.get_absolute(path)?;
        absolute_path.metadata().map(|m| {
            Box::new(Metadata(m)) as Box<VMetadata>
        }).map_err(GameError::from)
    }

    fn read_dir(&self, path: &Path) -> GameResult<fs::ReadDir> {
        let absolute_path = self.get_absolute(path)?;

        if absolute_path.is_dir() {
            match fs::read_dir(absolute_path.as_path()) {
                Ok(readdir) => Ok(readdir),
                Err(e) => Err(GameError::IOError(format!("Could not read the content of the directory at path ({})", absolute_path.display()), e))
            }
        } else {
            return Err(GameError::FileSystemError(format!("the path ({}) must be a directory !", absolute_path.display())));
        }
    }
}


//TODO: test the physical filesystem
#[cfg(test)]
mod linux_filesystem_test {
    use super::*;
    use std::io::BufReader;
    use std::io::Read;

    #[test]
    fn filesystem_mkdir() {
        let filesystem = Filesystem::new().unwrap();
        let mut dir_test = filesystem.root_directory();
        dir_test.push(Path::new("dir_test"));
        filesystem.mkdir(dir_test.as_path()).unwrap();
        assert!(filesystem.exists(dir_test.as_path()));

        filesystem.create(Path::new("dir_test/file_test.txt")).expect("Couldn't create file").write_all(b"text_test\n").expect("Couldn't create file and add 'text test'");
        filesystem.append(Path::new("dir_test/file_test.txt")).expect("Couldn't append to file").write_all(b"text_append_test\n").expect("Couldn't append to file and add 'text_append-test'");
        let mut bufreader = BufReader::new(filesystem.open(Path::new("dir_test/file_test.txt")).expect("Couldn't read file with bufreader"));
        let mut content = String::new();
        bufreader.read_to_string(&mut content);
        let mut lines = content.lines();
        println!("{:?}", content);
        assert_eq!(lines.next(), Some("text_test"));
        assert_eq!(lines.next(), Some("text_append_test"));
        assert_eq!(lines.next(), None);

        let file_metadata = filesystem.metadata(Path::new("dir_test/file_test.txt")).expect("Couldn't get metadata");
        assert!(file_metadata.is_file());
        assert!(!file_metadata.is_dir());
        assert!(!file_metadata.is_read_only());
        assert!(file_metadata.len() > 0);

        filesystem.create(Path::new("dir_test/file_test_rm.txt")).expect("Couldn't create file").write_all(b"test rm\n").expect("Coudln't create file and write test rm");
        filesystem.create(Path::new("dir_test/file_test_rm_2.txt")).expect("Couldn't create file").write_all(b"test rm 2\n").expect("Coudln't create file and write test rm 2");
        filesystem.rm(Path::new("dir_test/file_test_rm_2.txt")).expect("Couldn't delete the file : file_test_rm_2.txt");
        assert!(!filesystem.exists(Path::new("dir_test/file_test_rm_2.txt")));
        filesystem.rmrf(Path::new("dir_test")).expect("Couldn't delete dir");
        assert!(!filesystem.exists(Path::new("dir_test")));
    }


    #[test]
    fn filesystem_current_working_directory() {
        let filesystem = Filesystem::new().expect("Could not create FS");
        assert_eq!(env::current_dir().expect("Couldn't get the current working directory"), filesystem.root_directory());
    }


    #[test]
    fn filesystem_read_dir() {
        let filesystem = Filesystem::new().expect("Couldn't create FS");
        let mut entries = filesystem.read_dir(Path::new("src")).unwrap();
        assert!(entries.next().is_some()); //lib.rs
        assert!(entries.next().is_some()); //systems
        assert!(entries.next().is_some()); //gameplay
        assert!(entries.next().is_some()); //game_specific
        assert!(entries.next().is_some()); //core
        assert!(entries.next().is_none()); //nothing
    }

    #[test]
    fn filesystem_system_type() {
        let filesystem = Filesystem::new().expect("Couldn't create FS.");
        assert_eq!(filesystem.system_type(), SystemType::Filesystem);
    }

    #[test]
    fn filesystem_platofrm_type() {
        let filesystem = Filesystem::new().expect("Couldn't create FS.");
        assert_eq!(filesystem.platform(), PlatformType::Linux);
    }
}