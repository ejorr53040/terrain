/// A request to run a VM.
#[derive(serde::Serialize)]
pub struct RunRequest {
    pub image: String,
}

pub fn encode(request: &RunRequest) -> String {
    request.image.clone()
}
