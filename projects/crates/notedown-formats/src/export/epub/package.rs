//! Minimal EPUB 3 OCF ZIP packaging.
//!
//! Reuses the same stored (uncompressed) ZIP layout as the DOCX exporter so we
//! keep the dependency footprint free of a DEFLATE codec. EPUB readers accept
//! stored members; the spec only demands STORED for `mimetype` and does not
//! require compression elsewhere.

/// Build an EPUB ZIP archive from the given member entries.
///
/// Entries must be supplied in their final order — `mimetype` first so it can
/// be STORED and land at the archive start per EPUB 3 §4.3.6.
pub fn build_epub_package(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut central = Vec::new();

    for (path, payload) in entries {
        let name = path.as_bytes();
        let crc = crc32(payload);
        let local_offset = archive.len();

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&[
            0x14, 0x00, // version needed / flags (bit 0 = encrypted, bit 11 = utf-8 filename, bit 12 = data descriptor — all zero)
            0x00, 0x00, // compression: 0 = stored
            0x00, 0x00, // mod time
            0x00, 0x00, // mod date
        ]);
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&[0x00, 0x00]);
        archive.extend_from_slice(name);
        archive.extend_from_slice(payload);

        central.extend_from_slice(b"PK\x01\x02");
        // 42 bytes of fixed central directory header (APPNOTE total after sig = 46).
        let mut cd_fixed = [0u8; 42];
        cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]); // version made by
        cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]); // version needed
        cd_fixed[4..6].copy_from_slice(&[0x00, 0x00]); // flags
        cd_fixed[6..8].copy_from_slice(&[0x00, 0x00]); // compression
        cd_fixed[8..10].copy_from_slice(&[0x00, 0x00]); // mod time
        cd_fixed[10..12].copy_from_slice(&[0x00, 0x00]); // mod date
        cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
        cd_fixed[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
        cd_fixed[38..42].copy_from_slice(&(local_offset as u32).to_le_bytes());
        central.extend_from_slice(&cd_fixed);
        central.extend_from_slice(name);
    }

    let cd_offset = archive.len();
    archive.extend_from_slice(&central);
    let cd_size = central.len();
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let bit = crc & 1;
            crc >>= 1;
            if bit != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}
