use anywho::anywho;
use std::path::PathBuf;

pub fn read_qr_from_file(path: PathBuf) -> Result<String, anywho::Error> {
    let img = image::open(&path)?;
    let img = img.to_luma8();
    let mut img = rqrr::PreparedImage::prepare(img);
    let grids = img.detect_grids();

    // Get the first QR code found
    if let Some(grid) = grids.into_iter().next() {
        let (_meta, content) = grid.decode()?;
        Ok(content)
    } else {
        Err(anywho!("No QR code found in image"))
    }
}
