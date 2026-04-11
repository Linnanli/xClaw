use std::io::{Cursor, Read};

use crate::error::{Error, Result};

const SKILL_FILE_NAME: &str = "SKILL.md";
const MAX_SKILL_CONTENT_BYTES: usize = 64 * 1024;

pub fn extract_skill_content(file_name: &str, bytes: &[u8]) -> Result<String> {
    let lower_name = file_name.to_ascii_lowercase();

    if lower_name.ends_with(".zip") {
        return extract_from_zip(bytes);
    }

    if lower_name.ends_with(".tar.gz") || lower_name.ends_with(".tgz") {
        return extract_from_tar_gz(bytes);
    }

    Err(Error::Validation(
        "仅支持 .zip、.tar.gz、.tgz 技能包".into(),
    ))
}

fn extract_from_zip(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Validation(format!("zip 文件无效: {}", e)))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| Error::Validation(format!("读取 zip 条目失败: {}", e)))?;
        if !is_skill_file(file.name()) {
            continue;
        }

        return read_utf8_content(&mut file);
    }

    Err(Error::Validation("技能包中缺少 SKILL.md".into()))
}

fn extract_from_tar_gz(bytes: &[u8]) -> Result<String> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| Error::Validation(format!("tar.gz 文件无效: {}", e)))?;

    for entry in entries {
        let mut file = entry.map_err(|e| Error::Validation(format!("读取 tar 条目失败: {}", e)))?;
        let path = file
            .path()
            .map_err(|e| Error::Validation(format!("读取 tar 路径失败: {}", e)))?;
        let path_text = path.to_string_lossy();
        if !is_skill_file(&path_text) {
            continue;
        }

        return read_utf8_content(&mut file);
    }

    Err(Error::Validation("技能包中缺少 SKILL.md".into()))
}

fn is_skill_file(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|name| name == SKILL_FILE_NAME)
}

fn read_utf8_content<R: Read>(reader: &mut R) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_SKILL_CONTENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Validation(format!("SKILL.md 不是有效 UTF-8 文本: {}", e)))?;

    if bytes.len() > MAX_SKILL_CONTENT_BYTES {
        return Err(Error::Validation(
            "技能包中的 SKILL.md 超过 64 KiB 限制".into(),
        ));
    }

    String::from_utf8(bytes)
        .map_err(|e| Error::Validation(format!("SKILL.md 不是有效 UTF-8 文本: {}", e)))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};
    use zip::write::SimpleFileOptions;

    use super::extract_skill_content;

    #[test]
    fn extracts_skill_from_zip() {
        let mut zip_bytes = Cursor::new(Vec::new());
        {
            let mut zip_writer = zip::ZipWriter::new(&mut zip_bytes);
            let options = SimpleFileOptions::default();
            zip_writer
                .start_file("pkg/SKILL.md", options)
                .expect("start SKILL.md in zip");
            zip_writer
                .write_all(b"---\nname: zip-skill\nversion: 1.0.0\ndescription: test\nactivation:\n  keywords:\n    - zip\n---")
                .expect("write SKILL.md to zip");
            zip_writer.finish().expect("finish zip writer");
        }

        let content = extract_skill_content("skill.zip", zip_bytes.get_ref())
            .expect("zip extraction should succeed");
        assert!(content.contains("name: zip-skill"));
    }

    #[test]
    fn extracts_skill_from_tar_gz() {
        let mut tar_payload = Vec::new();
        {
            let encoder = GzEncoder::new(&mut tar_payload, Compression::default());
            let mut tar_builder = Builder::new(encoder);
            let skill = b"---\nname: tgz-skill\nversion: 1.0.0\ndescription: test\nactivation:\n  keywords:\n    - tgz\n---";

            let mut header = Header::new_gnu();
            header.set_size(skill.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();

            tar_builder
                .append_data(&mut header, "nested/SKILL.md", &skill[..])
                .expect("append SKILL.md to tar");

            tar_builder.finish().expect("finish tar builder");
        }

        let content = extract_skill_content("skill.tar.gz", &tar_payload)
            .expect("tar.gz extraction should succeed");
        assert!(content.contains("name: tgz-skill"));
    }

    #[test]
    fn rejects_archive_without_skill_file() {
        let mut zip_bytes = Cursor::new(Vec::new());
        {
            let mut zip_writer = zip::ZipWriter::new(&mut zip_bytes);
            zip_writer
                .start_file("README.md", SimpleFileOptions::default())
                .expect("start README in zip");
            zip_writer
                .write_all(b"no skill here")
                .expect("write README to zip");
            zip_writer.finish().expect("finish zip writer");
        }

        let error = extract_skill_content("skill.zip", zip_bytes.get_ref())
            .expect_err("should reject archive without SKILL.md");
        assert!(error.to_string().contains("缺少 SKILL.md"));
    }

    #[test]
    fn rejects_unsupported_extension() {
        let error = extract_skill_content("skill.md", b"---\nname: x\n---")
            .expect_err("should reject unsupported extension");
        assert!(error.to_string().contains("仅支持"));
    }

    #[test]
    fn rejects_skill_file_larger_than_limit() {
        let oversized = vec![b'a'; 64 * 1024 + 1];

        let mut zip_bytes = Cursor::new(Vec::new());
        {
            let mut zip_writer = zip::ZipWriter::new(&mut zip_bytes);
            zip_writer
                .start_file("SKILL.md", SimpleFileOptions::default())
                .expect("start SKILL.md in zip");
            zip_writer
                .write_all(&oversized)
                .expect("write oversized SKILL.md");
            zip_writer.finish().expect("finish zip writer");
        }

        let error = extract_skill_content("skill.zip", zip_bytes.get_ref())
            .expect_err("should reject oversized skill file");
        assert!(error.to_string().contains("64 KiB"));
    }
}