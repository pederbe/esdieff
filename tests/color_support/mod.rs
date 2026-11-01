pub fn plain(bytes: &[u8]) -> String {
    let mut text = String::from_utf8(bytes.to_vec()).unwrap();
    for sequence in [
        "\x1b[0m", "\x1b[1m", "\x1b[31m", "\x1b[32m", "\x1b[33m", "\x1b[36m",
    ] {
        text = text.replace(sequence, "");
    }
    text.replace("\r\n", "\n")
}
