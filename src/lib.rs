extern crate byteorder;

use std::io::{Read, Error};
use byteorder::*;

pub trait Format { }

impl Format for [u8; 4] { }

impl Format for [u8; 2] { }

impl Format for u8 { }

pub enum Document {
	Rgba(FormattedDocument<[u8; 4]>),
	Gray(FormattedDocument<[u8; 2]>),
	Indexed(FormattedDocument<u8>),
}

pub struct FormattedDocument<T: Format> {
	pub width: u16,
	pub height: u16,
	pub transparent_index: u8,

	pub frames: Vec<Frame<T>>,
	pub palette: Vec<[u8; 4]>,
}

pub struct Layer {
	//
}

pub struct Frame<T: Format> {
	pub duration: u16,
	pub chunks: Vec<Chunk<T>>,
}

pub enum CelData<T: Format> {
	Pixels {
		width: u16,
		height: u16,
		data: Vec<T>,
	}
}

pub enum FrameLoop {
	Forward,
	Reverse,
	PingPong,
}

pub struct FrameTag {
	pub from_frame: u16,
	pub to_frame: u16,
	pub loop_mode: FrameLoop,
	pub color: [u8; 3],
	pub name: String,
}

pub enum Chunk<T: Format> {
	Unsupported,

	Layer {
		flags: u16,
		is_group: bool,
		child_level: u16,
		width: u16,
		height: u16,
		blend: u16,
		opacity: u8,
		name: String,
	},

	Cel {
		layer_index: u16,
		x: u16,
		y: u16,
		opacity: u8,
		cel_type: u16,
		data: CelData<T>,
	},

	FrameTags {
		tags: Vec<FrameTag>,
	},

	Palette {
		//
	},

	UserData {
		//
	},

	Slice {
		//
	},
}

impl Document {
	pub fn new<R: Read>(from: &mut R) -> Result<Self, Error> {
		// start by reading the header data
		let _file_size = from.read_u32::<LE>()?;
		let _magic = from.read_u16::<LE>()?;
		let frame_count = from.read_u16::<LE>()?;
		let width = from.read_u16::<LE>()?;
		let height = from.read_u16::<LE>()?;
		let depth = from.read_u16::<LE>()?;
		let _flags = from.read_u32::<LE>()?;
		from.read_exact(&mut [0u8; 10])?;
		let transparent_index = from.read_u8()?;
		from.read_exact(&mut [0u8; 3])?;
		let _colors = from.read_u16::<LE>()?;
		from.read_exact(&mut [0u8; 94])?;

		Ok(match depth {
			32 => {
				// then load the frames
				let mut frames = vec![];
				let mut palette = vec![];
				for _ in 0..frame_count {
					// load frames
					let bytes = from.read_u32::<LE>()? as u64;
					frames.push(Frame::new(&mut from.take(bytes), &mut palette)?);
				}

				Document::Rgba(FormattedDocument {
					width,
					height,
					transparent_index,
					frames,
					palette,
				})
			},

			16 => {
				// then load the frames
				let mut frames = vec![];
				let mut palette = vec![];
				for _ in 0..frame_count {
					// load frames
					let bytes = from.read_u32::<LE>()? as u64;
					frames.push(Frame::new(&mut from.take(bytes), &mut palette)?);
				}

				Document::Gray(FormattedDocument {
					width,
					height,
					transparent_index,
					frames,
					palette,
				})
			},

			8 => {
				// then load the frames
				let mut frames = vec![];
				let mut palette = vec![];
				for _ in 0..frame_count {
					// load frames
					let bytes = from.read_u32::<LE>()? as u64;
					frames.push(Frame::new(&mut from.take(bytes), &mut palette)?);
				}

				Document::Indexed(FormattedDocument {
					width,
					height,
					transparent_index,
					frames,
					palette,
				})
			},

			_ => panic!(),
		})
	}
}

impl<T: Format> Frame<T> {
	pub fn new<R: Read>(from: &mut R, palette: &mut Vec<[u8; 4]>) -> Result<Self, Error> {
		// start by reading the frame header
		let _magic = from.read_u16::<LE>()?;
		let old_chunks = from.read_u16::<LE>()? as u32;
		let duration = from.read_u16::<LE>()?;
		from.read_u16::<LE>()?;
		let mut chunk_count = from.read_u32::<LE>()?;
		if chunk_count == 0 {
			chunk_count = old_chunks;
		}

		// then load the chunks
		let mut chunks = vec![];
		for _ in 0..chunk_count {
			let bytes = from.read_u32::<LE>()? as u64;
			chunks.push(Chunk::new(&mut from.take(bytes))?);
		}

		Ok(Frame {
			duration,
			chunks,
		})
	}
}

fn read_string<R: Read>(from: &mut R) -> Result<String, Error> {
	let length = from.read_u16::<LE>()?;
	let mut bytes = Vec::with_capacity(length as _);
	for _ in 0..length {
		bytes.push(from.read_u8()?);
	}
	Ok(String::from_utf8(bytes).unwrap())
}

impl<T: Format> Chunk<T> {
	pub fn new<R: Read>(from: &mut R) -> Result<Self, Error> {
		let chunk_type = from.read_u16::<LE>()?;
		Ok(match chunk_type {
			0x0004 => {
				Chunk::Palette {
					//
				}
			},
			0x0011 => {
				Chunk::Palette {
					//
				}
			},
			0x2004 => {
				Chunk::Layer {
					flags: from.read_u16::<LE>()?,
					is_group: match from.read_u8()? { 0 => false, _ => true },
					child_level: from.read_u16::<LE>()?,
					width: from.read_u16::<LE>()?,
					height: from.read_u16::<LE>()?,
					blend: from.read_u16::<LE>()?,
					opacity: from.read_u8()?,
					name: read_string(from)?,
				}
			},
			0x2005 => {
				Chunk::Cel {
					//
				}
			},
			0x2006 => {
				Chunk::CelExtra {
					//
				}
			},
			0x2018 => {
				Chunk::FrameTags {
					//
				}
			},
			0x2019 => {
				Chunk::Palette {
					//
				}
			},
			0x2020 => {
				Chunk::UserData {
					//
				}
			},
			0x2022 => {
				Chunk::Slice {
					//
				}
			},
			_ => Chunk::Unsupported,
		})
	}
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
