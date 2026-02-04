use std::io::Write;
use zip::write::FileOptions;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_zip_import_logic() {
        // This test simulates creating a zip file and verifying we can read it using the same logic as the command
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        let file = File::create(&zip_path).unwrap();

        let mut zip = zip::ZipWriter::new(file);
        let options: FileOptions<()> =
            FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("data.sql", options).unwrap();
        zip.write_all(b"INSERT INTO test VALUES (1);").unwrap();
        zip.finish().unwrap();

        // Now try to read it back
        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut sql_file_index = None;
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            if file.name().ends_with(".sql") {
                sql_file_index = Some(i);
                break;
            }
        }

        assert!(sql_file_index.is_some());

        let mut content = Vec::new();
        let mut file = archive.by_index(sql_file_index.unwrap()).unwrap();
        std::io::Read::read_to_end(&mut file, &mut content).unwrap();

        let sql = String::from_utf8(content).unwrap();
        assert_eq!(sql, "INSERT INTO test VALUES (1);");
    }
}
