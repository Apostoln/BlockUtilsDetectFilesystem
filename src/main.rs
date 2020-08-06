use duct::*;
use std::fs::*;
use std::path::*;
use std::str::FromStr;

pub const LEN_1_KB: u64 = 1 * 1024;
pub const LEN_100_MB: u64 = 100 * 1024 * LEN_1_KB;
const TESTS_NUM: u64 = 100;

fn main() {
    // Run test many times for detecting bug
    for i in 0..TESTS_NUM {
        println!("Test {}/{}", i, TESTS_NUM);
        test();
    }
}

fn test() {
    const IMG_FILE_SIZE: u64= LEN_100_MB;
    const FS_TYPE: &str = "ext2";
    const BLOCK_SIZE: u64 = LEN_1_KB;

    let partition = TestLoopPartition::new(IMG_FILE_SIZE, FS_TYPE, BLOCK_SIZE);
    let fs = get_fs(&partition.partition_name);
    if let block_utils::FilesystemType::Unknown = fs {
        // Fs is detected as unknown
        // That's already wrong, but let's compare it with `blkid` result

        let blkid_out = cmd!("blkid", "-o", "udev", &partition.partition_name).stdout_capture().read().unwrap();
        // output example:
        // /dev/loop43p1: UUID="d5ddce78-20d7-4c09-84c2-48b0d6ea6a38" TYPE="ext2" PARTUUID="92afa785-01"

        // get filesystem type from output string
        let blkid_fs_str = blkid_out.lines().skip(2).next().unwrap().split("=").skip(1).next().unwrap();
        let blkid_fs = block_utils::FilesystemType::from_str(blkid_fs_str).unwrap();

        println!("{}", partition.partition_name);

        // blkid_fs is the same as partition.fs_type, which is `ext2`
        assert_eq!(blkid_fs, block_utils::FilesystemType::from_str(&partition.fs_type).unwrap());

        // But it's not the same as fs detected with block-utils
        assert_eq!(fs, blkid_fs); // fail
    }
}

fn get_fs(partition: &str) -> block_utils::FilesystemType {
    let fs = block_utils::get_device_from_path(PathBuf::from(&partition).as_path())
        .unwrap()
        .1
        .unwrap()
        .fs_type;
    fs
}

// Almost the same as viy/tests/utils/TestLoopPartition
#[derive(Default, Debug)]
pub struct TestLoopPartition {
    partition_name: String,
    loop_device_mount_point: String,
    loop_device_img_file: PathBuf,
    block_size: u64,
    tmp_folder: String,
    loop_device_name: String,
    fs_type: String,
}

impl TestLoopPartition {
    pub fn new(
        img_file_size: u64,
        fs_type: &str,
        block_size: u64,
    ) -> Self {
        /* Steps:
           1. get next available loop device name
           2. create img file for a loop device
           3. create mount path for a loop device
           4. create loop device
           5. make partition label on loop device
           6. make partition
           7. make fs on partition
           8. mount partition
        */

        // 1. get next available loop device name
        let loop_device =
            cmd!("losetup", "-f")
                .read()
                .unwrap();

        // 2. create img file for a loop device
        let img_file_path =
            PathBuf::from(".").join(format!("{}.img", &loop_device.replace("/dev/", "")));
        let file = File::create(&img_file_path).unwrap();
        file.set_len(img_file_size).unwrap();

        // 3. create mount path for a loop device
        let tmp_folder = String::from(format!(
            "/tmp/throw_away_test/{}_tmp",
            loop_device.replace("/dev/", "")
        ));
        let loop_mount_point = format!("{}/{}", tmp_folder, loop_device.replace("/dev/", ""));
        create_dir_all(&loop_mount_point).unwrap();

        // 4. create loop device
        cmd!("losetup", &loop_device, &img_file_path)
            .stdout_null()
            .run()
            .unwrap();

        // 5. make partition label on loop device
        cmd!("parted", "-s", &loop_device, "mklabel msdos")
            .stdout_null()
            .stderr_null()
            .run()
            .unwrap();

        // 6. make partition
        let img_size_mb = img_file_size / 1024 / 1024;
        cmd!(
            "parted",
            "-s",
            &loop_device,
            "mkpart primary 0",
            img_size_mb.to_string()
        )
            .stderr_null()
            .stdout_null()
            .run()
            .unwrap();
        let partition_name = format!("{}p1", &loop_device);

        // 7. make fs on partition
        let mkfs_command = format!("mkfs.{}", fs_type);
        cmd!(
                &mkfs_command,
                "-F",
                &partition_name,
                "-b",
                block_size.to_string()
            )
            .stdout_null() // If you comment out these lines, then there will be almost no errors
            .stderr_null() //
            .run()
            .unwrap();

        // It doesn't work:
        // std::thread::sleep(std::time::Duration::from_secs(10));


        TestLoopPartition {
            tmp_folder,
            partition_name,
            loop_device_name: loop_device,
            loop_device_mount_point: loop_mount_point,
            loop_device_img_file: img_file_path,
            fs_type: String::from(fs_type),
            block_size,
        }
    }
}

impl Drop for TestLoopPartition {
    fn drop(&mut self) {
        cmd!("parted", "-s", &self.loop_device_name, "rm", "1")
            .stdout_null()
            .run()
            .unwrap();
        cmd!("losetup", "-d", &self.loop_device_name)
            .stdout_null()
            .run()
            .unwrap();
        remove_file(&self.loop_device_img_file).unwrap();
        remove_dir_all(&self.tmp_folder).unwrap();
    }
}