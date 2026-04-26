use std::io::{Cursor, Read};

use crate::error::{Error, Result};

const SKILL_FILE_NAME: &str = "SKILL.md";
const PLATFORM_MANIFEST_FILE_NAME: &str = "manifest.json";
const MAX_SKILL_CONTENT_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_CONTENT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct ExtractedSkillPackage {
    pub skill_content: String,
    pub manifest_json: Option<String>,
}

pub fn extract_skill_package(file_name: &str, bytes: &[u8]) -> Result<ExtractedSkillPackage> {
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

pub fn extract_skill_content(file_name: &str, bytes: &[u8]) -> Result<String> {
    extract_skill_package(file_name, bytes).map(|package| package.skill_content)
}

fn extract_from_zip(bytes: &[u8]) -> Result<ExtractedSkillPackage> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Validation(format!("zip 文件无效: {}", e)))?;
    let mut skill_content: Option<String> = None;
    let mut manifest_json: Option<String> = None;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| Error::Validation(format!("读取 zip 条目失败: {}", e)))?;
        let path = normalize_archive_path(file.name());

        if skill_content.is_none() && is_skill_file(&path) {
            skill_content = Some(read_utf8_content_with_limit(
                &mut file,
                MAX_SKILL_CONTENT_BYTES,
                "SKILL.md",
            )?);
            continue;
        }

        if manifest_json.is_none() && is_platform_manifest_file(&path) {
            manifest_json = Some(read_utf8_content_with_limit(
                &mut file,
                MAX_MANIFEST_CONTENT_BYTES,
                PLATFORM_MANIFEST_FILE_NAME,
            )?);
        }
    }

    let skill_content =
        skill_content.ok_or_else(|| Error::Validation("技能包中缺少 SKILL.md".into()))?;

    Ok(ExtractedSkillPackage {
        skill_content,
        manifest_json,
    })
}

fn extract_from_tar_gz(bytes: &[u8]) -> Result<ExtractedSkillPackage> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| Error::Validation(format!("tar.gz 文件无效: {}", e)))?;
    let mut skill_content: Option<String> = None;
    let mut manifest_json: Option<String> = None;

    for entry in entries {
        let mut file = entry.map_err(|e| Error::Validation(format!("读取 tar 条目失败: {}", e)))?;
        let path = file
            .path()
            .map_err(|e| Error::Validation(format!("读取 tar 路径失败: {}", e)))?;
        let path_text = normalize_archive_path(&path.to_string_lossy());

        if skill_content.is_none() && is_skill_file(&path_text) {
            skill_content = Some(read_utf8_content_with_limit(
                &mut file,
                MAX_SKILL_CONTENT_BYTES,
                "SKILL.md",
            )?);
            continue;
        }

        if manifest_json.is_none() && is_platform_manifest_file(&path_text) {
            manifest_json = Some(read_utf8_content_with_limit(
                &mut file,
                MAX_MANIFEST_CONTENT_BYTES,
                PLATFORM_MANIFEST_FILE_NAME,
            )?);
        }
    }

    let skill_content =
        skill_content.ok_or_else(|| Error::Validation("技能包中缺少 SKILL.md".into()))?;

    Ok(ExtractedSkillPackage {
        skill_content,
        manifest_json,
    })
}

fn is_skill_file(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name == SKILL_FILE_NAME)
}

fn is_platform_manifest_file(path: &str) -> bool {
    if path == PLATFORM_MANIFEST_FILE_NAME {
        return true;
    }

    let mut segments = path.split('/');
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(_), Some(PLATFORM_MANIFEST_FILE_NAME), None)
    )
}

fn normalize_archive_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn read_utf8_content_with_limit<R: Read>(
    reader: &mut R,
    max_bytes: usize,
    file_label: &str,
) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Validation(format!("{} 不是有效 UTF-8 文本: {}", file_label, e)))?;

    if bytes.len() > max_bytes {
        return Err(Error::Validation(format!(
            "技能包中的 {} 超过 {} KiB 限制",
            file_label,
            max_bytes / 1024
        )));
    }

    String::from_utf8(bytes)
        .map_err(|e| Error::Validation(format!("{} 不是有效 UTF-8 文本: {}", file_label, e)))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};
    use zip::write::SimpleFileOptions;

    use super::{extract_skill_content, extract_skill_package};

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
    fn extracts_manifest_json_from_zip() {
        let mut zip_bytes = Cursor::new(Vec::new());
        {
            let mut zip_writer = zip::ZipWriter::new(&mut zip_bytes);
            let options = SimpleFileOptions::default();
            zip_writer
                .start_file("pkg/SKILL.md", options)
                .expect("start SKILL.md in zip");
            zip_writer
                .write_all(
                    b"---\nname: zip-skill\ndescription: test\nactivation:\n  keywords:\n    - zip\n---",
                )
                .expect("write SKILL.md to zip");
            zip_writer
                .start_file("pkg/manifest.json", options)
                .expect("start manifest.json in zip");
            zip_writer
                .write_all(br#"{"version":"2.3.4"}"#)
                .expect("write manifest.json to zip");
            zip_writer.finish().expect("finish zip writer");
        }

        let package = extract_skill_package("skill.zip", zip_bytes.get_ref())
            .expect("zip extraction should succeed");
        assert!(package.skill_content.contains("name: zip-skill"));
        assert_eq!(
            package.manifest_json.as_deref(),
            Some("{\"version\":\"2.3.4\"}")
        );
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
