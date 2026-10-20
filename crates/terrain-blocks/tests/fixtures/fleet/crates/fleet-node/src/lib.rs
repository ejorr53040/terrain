mod vm;

use fleet_proto::RunRequest;

pub fn serve() {
    tokio::spawn(async {});
}

pub fn digest(request: &RunRequest) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(request.image.as_bytes()).to_vec()
}

pub const PORT: u16 = 7070;
