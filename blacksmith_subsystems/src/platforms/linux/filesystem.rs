

use std::fs;
use std::path::{Path, PathBuf};
use std::fmt;
use std::io;

use std::env;

use remove_dir_all;

use blacksmith_core::game_infos::GameInfos;
use blacksmith_core::game_directories::GameDirectories;

use blacksmith_core::engine_support_systems::system_interfaces::filesystems::{VFilesystem, VMetadata, VFile, OpenOptions, RootDir};
use blacksmith_core::engine_support_systems::error_handling::error::{GameResult, GameError};
use blacksmith_core::engine_support_systems::system_interfaces::System;
use blacksmith_core::engine_support_systems::system_interfaces::SystemType;
use blacksmith_core::engine_support_systems::system_interfaces::PlatformType;

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
    game_infos: GameInfos,
    game_directories: GameDirectories,
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
    pub fn new(game_infos: GameInfos) -> GameResult<Filesystem> {

        let game_directories = GameDirectories::new(&game_infos)?;

        let filesystem = Filesystem {
            game_infos,
            game_directories
        };

        //TODO: make it explicite, our generate the structure when actuallt writing to it.
        filesystem.mkdir(RootDir::UserEngineConfigurationRoot, "")?;
        filesystem.mkdir(RootDir::UserLogRoot, "")?;
        filesystem.mkdir(RootDir::UserSaveRoot, "")?;

        Ok(filesystem)
    }

    fn get_root_directory(&self, root_dir: RootDir) -> &PathBuf {
        match root_dir {
            RootDir::UserConfigRoot => {
                &self.game_directories.user_config_path()
            },
            RootDir::UserDataRoot => {
                &self.game_directories.user_data_path()
            },
            RootDir::UserEngineConfigurationRoot => {
                &self.game_directories.engine_configuration_path()
            },
            RootDir::UserLogRoot => {
                &self.game_directories.logs_path()
            },
            RootDir::WorkingDirectory => {
                &self.game_directories.current_path()
            },
            RootDir::UserSaveRoot => {
                &self.game_directories.saves_path()
            }
        }
    }

    fn get_absolute_path(&self, root_dir: RootDir, path: &str) -> PathBuf {
        let mut root = self.get_root_directory(root_dir).clone();
        //An empty &str can be used to delete a root directory (for tests). A bit hacky but....
        if !path.is_empty() {
            root.push(Path::new(path));
        }
        root
    }
}

impl VFilesystem for Filesystem {

    fn application_info(&self) -> &GameInfos {
        &self.game_infos
    }

    fn open_with_options(&self, root_dir: RootDir, path: &str, open_options: &OpenOptions) -> GameResult<Box<VFile>> {
        let absolute_path = self.get_absolute_path(root_dir, path);

        open_options
            .to_fs_openoptions()
            .open(absolute_path.as_path())
            .map(|file| Box::new(file) as Box<VFile>).
            map_err(GameError::from)
    }

    fn mkdir(&self, root_dir: RootDir, path: &str) -> GameResult<()> {
        let absolute_path = self.get_absolute_path(root_dir, path);
        println!("creating directory {}", absolute_path.display());
        fs::DirBuilder::new().recursive(true).create(absolute_path.as_path()).map_err(GameError::from)
    }

    fn rm(&self, root_dir: RootDir, path: &str) -> GameResult<()> {
        let absolute_path = self.get_absolute_path(root_dir, path);
        if absolute_path.is_dir() {
            fs::remove_dir(path).map_err(GameError::from)
        } else {
            fs::remove_file(path).map_err(GameError::from)
        }
    }

    fn rmrf(&self, root_dir: RootDir, path: &str) -> GameResult<()> {
        if self.exists(root_dir, path) {
            let absolute_path = self.get_absolute_path(root_dir, path);
            remove_dir_all::remove_dir_all(absolute_path.as_path()).map_err(GameError::from)
        } else {
            Ok(())
        }
    }

    fn exists(&self, root_dir: RootDir, path: &str) -> bool {
        self.get_absolute_path(root_dir, path).exists()
    }

    fn metadata(&self, root_dir: RootDir, path: &str) -> GameResult<Box<VMetadata>> {
        let absolute_path = self.get_absolute_path(root_dir, path);
        absolute_path.metadata().map(|m| {
            Box::new(Metadata(m)) as Box<VMetadata>
        }).map_err(GameError::from)
    }

    fn read_dir(&self, root_dir: RootDir, path: &str) -> GameResult<fs::ReadDir> {
        let absolute_path = self.get_absolute_path(root_dir, path);

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

#[cfg(test)]
mod linux_filesystem_test {
    use super::*;
    use std::io::BufReader;
    use std::io::Read;

    #[test]
    fn filesystem_io_operations() {
        let filesystem = Filesystem::new(GameInfos::new("test_filesystem_blacksmith", "Malkaviel")).expect("Couldn't create FS");

        filesystem.mkdir(RootDir::WorkingDirectory, "dir_test").unwrap();
        assert!(filesystem.exists(RootDir::WorkingDirectory, "dir_test"));

        //user logs
        filesystem.mkdir(RootDir::UserLogRoot, "log_dir_test").unwrap();
        assert!(filesystem.exists(RootDir::UserLogRoot, "log_dir_test"));
        filesystem.create(RootDir::UserLogRoot, "log_dir_test/file_test.txt").expect("Couldn't create file in user log dir").write_all(b"text_test\n").expect("Couldn't create file and add 'text test'");
        filesystem.append(RootDir::UserLogRoot, "log_dir_test/file_test.txt").expect("Couldn't append to file in user log dir").write_all(b"text_append_test\n").expect("Couldn't append to file and add 'text_append-test'");
        //user data
        filesystem.mkdir(RootDir::UserDataRoot, "user_data_dir_test");
        assert!(filesystem.exists(RootDir::UserDataRoot, "user_data_dir_test"));
        filesystem.create(RootDir::UserDataRoot, "user_data_dir_test/file_test.txt").expect("Couldn't create file in user log dir").write_all(b"text_test\n").expect("Couldn't create file and add 'text test'");
        filesystem.append(RootDir::UserDataRoot, "user_data_dir_test/file_test.txt").expect("Couldn't append to file in user log dir").write_all(b"text_append_test\n").expect("Couldn't append to file and add 'text_append-test'");
        //user engine config
        filesystem.mkdir(RootDir::UserEngineConfigurationRoot, "engine_conf_dir_test");
        assert!(filesystem.exists(RootDir::UserEngineConfigurationRoot, "engine_conf_dir_test"));
        filesystem.create(RootDir::UserEngineConfigurationRoot, "engine_conf_dir_test/file_test.txt").expect("Couldn't create file in user log dir").write_all(b"text_test\n").expect("Couldn't create file and add 'text test'");
        filesystem.append(RootDir::UserEngineConfigurationRoot, "engine_conf_dir_test/file_test.txt").expect("Couldn't append to file in user log dir").write_all(b"text_append_test\n").expect("Couldn't append to file and add 'text_append-test'");
        //user config
        filesystem.mkdir(RootDir::UserConfigRoot, "user_config_dir_test");
        assert!(filesystem.exists(RootDir::UserConfigRoot, "user_config_dir_test"));
        filesystem.create(RootDir::UserConfigRoot, "user_config_dir_test/file_test.txt").expect("Couldn't create file in user log dir").write_all(b"text_test\n").expect("Couldn't create file and add 'text test'");
        filesystem.append(RootDir::UserConfigRoot, "user_config_dir_test/file_test.txt").expect("Couldn't append to file in user log dir").write_all(b"text_append_test\n").expect("Couldn't append to file and add 'text_append-test'");
        //working dir
        filesystem.create(RootDir::WorkingDirectory, "dir_test/file_test.txt").expect("Couldn't create file").write_all(b"text_test\n").expect("Couldn't create file and add 'text test'");
        filesystem.append(RootDir::WorkingDirectory, "dir_test/file_test.txt").expect("Couldn't append to file").write_all(b"text_append_test\n").expect("Couldn't append to file and add 'text_append-test'");
        let mut bufreader = BufReader::new(filesystem.open(RootDir::WorkingDirectory, "dir_test/file_test.txt").expect("Couldn't read file with bufreader"));
        let mut content = String::new();
        bufreader.read_to_string(&mut content);
        let mut lines = content.lines();
        assert_eq!(lines.next(), Some("text_test"));
        assert_eq!(lines.next(), Some("text_append_test"));
        assert_eq!(lines.next(), None);

        //Metadata
        let file_metadata = filesystem.metadata(RootDir::WorkingDirectory, "dir_test/file_test.txt").expect("Couldn't get metadata");
        assert!(file_metadata.is_file());
        assert!(!file_metadata.is_dir());
        assert!(!file_metadata.is_read_only());
        assert!(file_metadata.len() > 0);

        //remove
        filesystem.create(RootDir::WorkingDirectory, "dir_test/file_test_rm.txt").expect("Couldn't create file").write_all(b"test rm\n").expect("Coudln't create file and write test rm");
        filesystem.create(RootDir::WorkingDirectory, "dir_test/file_test_rm_2.txt").expect("Couldn't create file").write_all(b"test rm 2\n").expect("Coudln't create file and write test rm 2");
        filesystem.rm(RootDir::WorkingDirectory, "dir_test/file_test_rm_2.txt").expect("Couldn't delete the file : file_test_rm_2.txt");
        assert!(!filesystem.exists(RootDir::WorkingDirectory, "dir_test/file_test_rm_2.txt"));
        filesystem.rmrf(RootDir::WorkingDirectory, "dir_test").expect("Couldn't delete dir");
        assert!(!filesystem.exists(RootDir::WorkingDirectory, "dir_test"));

        filesystem.rmrf(RootDir::UserConfigRoot, "").expect("Couldn't delete user config root dir");
        filesystem.rmrf(RootDir::UserDataRoot, "").expect("Couldn't delete user data root dir");
        assert!(!filesystem.exists(RootDir::UserConfigRoot, "user_config_dir_test"));
        assert!(!filesystem.exists(RootDir::UserDataRoot, "user_data_dir_test"));

    }


    #[test]
    fn filesystem_directories() {
        let filesystem = Filesystem::new(GameInfos::new("test_filesystem_blacksmith", "Malkaviel")).expect("Couldn't create FS");
        assert_eq!(&env::current_dir().expect("Couldn't get the current working directory"), filesystem.get_root_directory(RootDir::WorkingDirectory));
    }


    #[test]
    fn filesystem_read_dir() {
        let filesystem = Filesystem::new(GameInfos::new("test_filesystem_blacksmith", "Malkaviel")).expect("Couldn't create FS");
        let mut entries = filesystem.read_dir(RootDir::WorkingDirectory, "src").unwrap();
        assert!(entries.next().is_some()); //lib.rs
        assert!(entries.next().is_some()); //game.rs
        assert!(entries.next().is_none());
    }

    #[test]
    fn filesystem_system_type() {
        let filesystem = Filesystem::new(GameInfos::new("test_filesystem_blacksmith", "Malkaviel")).expect("Couldn't create FS");
        assert_eq!(filesystem.system_type(), SystemType::Filesystem);
    }

    #[test]
    fn filesystem_platform_type() {
        let filesystem = Filesystem::new(GameInfos::new("test_filesystem_blacksmith", "Malkaviel")).expect("Couldn't create FS");
        assert_eq!(filesystem.platform(), PlatformType::Linux);
    }
}