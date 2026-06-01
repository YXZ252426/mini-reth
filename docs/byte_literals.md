 Rust byte literals

  b'a'
  one byte, type u8

  b"abc"
  byte string, type &'static [u8; 3]

  b"\x12\x34"
  raw bytes, same content as [0x12, 0x34]

  "abc"
  UTF-8 string slice, type &'static str

  b"value".to_vec()
  copies byte string into Vec<u8>

  Examples

  b'a' == 97u8

  b"value" == [118, 97, 108, 117, 101]

  b"\x12\x34" == [0x12, 0x34]

  Short version

  b means treating the literal as bytes, not as str.