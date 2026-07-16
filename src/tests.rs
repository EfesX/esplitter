use crate::*;
use tempfile::tempdir;

fn create_test_file(dir: &tempfile::TempDir, name: &str, size: usize, fill_byte: u8) -> PathBuf {
    let path = dir.path().join(name);
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(&vec![fill_byte; size]).unwrap();
    path
}

#[test]
fn test_split_file_not_found() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("non_existent.txt");

    let result = split(&path, 100);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[test]
fn test_split_part_size_zero() {
    let dir = tempdir().unwrap();
    let path = create_test_file(&dir, "test.txt", 100, b'A');

    let result = split(&path, 0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn test_split_empty_file() {
    let dir = tempdir().unwrap();
    let path = create_test_file(&dir, "empty.txt", 0, b'A');

    let result = split(&path, 100);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn test_split_no_split_needed() {
    let dir = tempdir().unwrap();
    let path = create_test_file(&dir, "small.txt", 50, b'B');

    let result = split(&path, 100);
    assert!(result.is_ok());
    assert!(!dir.path().join("small.txt.part.001").exists());
}

#[test]
fn test_split_basic() {
    let dir = tempdir().unwrap();
    let path = create_test_file(&dir, "data.bin", 1000, b'X');

    let result = split(&path, 300);
    assert!(result.is_ok());

    let p1 = dir.path().join("data.bin.part.001");
    let p2 = dir.path().join("data.bin.part.002");
    let p3 = dir.path().join("data.bin.part.003");
    let p4 = dir.path().join("data.bin.part.004");
    let p5 = dir.path().join("data.bin.part.005");

    assert!(p1.exists() && fs::metadata(&p1).unwrap().len() == 300);
    assert!(p2.exists() && fs::metadata(&p2).unwrap().len() == 300);
    assert!(p3.exists() && fs::metadata(&p3).unwrap().len() == 300);
    assert!(p4.exists() && fs::metadata(&p4).unwrap().len() == 100);
    assert!(!p5.exists());

    let mut reassembled = Vec::new();
    reassembled.extend(fs::read(&p1).unwrap());
    reassembled.extend(fs::read(&p2).unwrap());
    reassembled.extend(fs::read(&p3).unwrap());
    reassembled.extend(fs::read(&p4).unwrap());

    assert_eq!(reassembled, vec![b'X'; 1000]);
}

#[test]
fn test_split_part_size_smaller_than_buffer() {
    let dir = tempdir().unwrap();
    let part_size = 10 * 1024;
    let total_size = 25 * 1024;
    let path = create_test_file(&dir, "buffer_test.bin", total_size, b'Y');

    let result = split(&path, part_size);
    assert!(result.is_ok());

    assert_eq!(
        fs::metadata(dir.path().join("buffer_test.bin.part.001"))
            .unwrap()
            .len() as usize,
        part_size
    );
    assert_eq!(
        fs::metadata(dir.path().join("buffer_test.bin.part.002"))
            .unwrap()
            .len() as usize,
        part_size
    );
    assert_eq!(
        fs::metadata(dir.path().join("buffer_test.bin.part.003"))
            .unwrap()
            .len() as usize,
        5 * 1024
    );
}

#[test]
fn test_split_part_size_larger_than_buffer() {
    let dir = tempdir().unwrap();
    let part_size = 100 * 1024;
    let total_size = 250 * 1024;
    let path = create_test_file(&dir, "large_part_test.bin", total_size, b'Z');

    let result = split(&path, part_size);
    assert!(result.is_ok());

    assert_eq!(
        fs::metadata(dir.path().join("large_part_test.bin.part.001"))
            .unwrap()
            .len() as usize,
        part_size
    );
    assert_eq!(
        fs::metadata(dir.path().join("large_part_test.bin.part.002"))
            .unwrap()
            .len() as usize,
        part_size
    );
    assert_eq!(
        fs::metadata(dir.path().join("large_part_test.bin.part.003"))
            .unwrap()
            .len() as usize,
        50 * 1024
    );
}

#[test]
fn test_merge_basic() {
    let dir = tempdir().unwrap();
    let original_path = dir.path().join("original.dat");

    // Create source file and split it
    // Fixed: create vector correctly
    let mut data = Vec::new();
    data.extend_from_slice(&[b'A'; 500]);
    data.extend_from_slice(&[b'B'; 500]);
    data.extend_from_slice(&[b'C'; 500]);

    let mut file = fs::File::create(&original_path).unwrap();
    file.write_all(&data).unwrap();

    split(&original_path, 500).unwrap();

    // Remove original file to verify that merge restores it
    fs::remove_file(&original_path).unwrap();
    assert!(!original_path.exists());

    // Perform merge
    let result = merge(&original_path, false);
    assert!(result.is_ok());

    // Check that file is restored and data is identical
    assert!(original_path.exists());
    let merged_data = fs::read(&original_path).unwrap();
    assert_eq!(merged_data, data);

    // Check that parts are still in place (if you didn't uncomment their deletion)
    assert!(dir.path().join("original.dat.part.001").exists());
}

#[test]
fn test_merge_no_parts_found() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.dat");

    let result = merge(&path, true);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
}

#[test]
fn test_merge_out_of_order_creation() {
    let dir = tempdir().unwrap();
    let original_path = dir.path().join("test_order.bin");

    // Create parts manually in reverse order to check sorting
    fs::write(dir.path().join("test_order.bin.part.003"), vec![b'C'; 100]).unwrap();
    fs::write(dir.path().join("test_order.bin.part.001"), vec![b'A'; 100]).unwrap();
    fs::write(dir.path().join("test_order.bin.part.002"), vec![b'B'; 100]).unwrap();

    let result = merge(&original_path, true);
    assert!(result.is_ok());

    let merged_data = fs::read(&original_path).unwrap();
    let expected = [vec![b'A'; 100], vec![b'B'; 100], vec![b'C'; 100]].concat();
    assert_eq!(merged_data, expected);
}
