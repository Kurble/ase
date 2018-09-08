extern crate byteorder;
extern crate libflate;

use std::io::{Read, Error, ErrorKind};
use byteorder::*;
use libflate::zlib::Decoder;

pub trait Format: Sized { 
	fn read<R: Read>(&mut R) -> Result<Self, Error>;
}

impl Format for [u8; 4] { 
	fn read<R: Read>(from: &mut R) -> Result<Self, Error> {
		Ok([from.read_u8()?, from.read_u8()?, from.read_u8()?, from.read_u8()?])
	}
}

impl Format for [u8; 2] {
	fn read<R: Read>(from: &mut R) -> Result<Self, Error> {
		Ok([from.read_u8()?, from.read_u8()?])
	}
}

impl Format for u8 { 
	fn read<R: Read>(from: &mut R) -> Result<Self, Error> {
		Ok(from.read_u8()?)
	}
}

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
}

pub struct ColorEntry {
	pub color: [u8; 4],
	pub name: Option<String>,
}

pub struct Frame<T: Format> {
	pub duration: u16,
	pub chunks: Vec<Chunk<T>>,
}

pub struct Cel<T: Format> {
	pub x: u16,
	pub y: u16,
	pub opacity: u8,
	pub data: CelData<T>,
}

pub enum CelData<T: Format> {
	Pixels {
		width: u16,
		height: u16,
		data: Vec<T>,
	},

	Link {
		frame: u16,
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
		cel: Option<Cel<T>>,
	},

	Cel {
		layer_index: u16,
		cel: Cel<T>,
	},

	FrameTags {
		tags: Vec<FrameTag>,
	},

	Palette {
		new_size: u32,
		first: u32,
		last: u32,
		updates: Vec<ColorEntry>,
	},

	UserData {
		text: Option<String>,
		color: Option<[u8; 4]>,
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
				for _ in 0..frame_count {
					// load frames
					let bytes = from.read_u32::<LE>()? as u64;
					frames.push(Frame::new(&mut from.take(bytes))?);
				}

				Document::Rgba(FormattedDocument {
					width,
					height,
					transparent_index,
					frames,
				})
			},

			16 => {
				// then load the frames
				let mut frames = vec![];
				for _ in 0..frame_count {
					// load frames
					let bytes = from.read_u32::<LE>()? as u64;
					frames.push(Frame::new(&mut from.take(bytes))?);
				}

				Document::Gray(FormattedDocument {
					width,
					height,
					transparent_index,
					frames,
				})
			},

			8 => {
				// then load the frames
				let mut frames = vec![];
				for _ in 0..frame_count {
					// load frames
					let bytes = from.read_u32::<LE>()? as u64;
					frames.push(Frame::new(&mut from.take(bytes))?);
				}

				Document::Indexed(FormattedDocument {
					width,
					height,
					transparent_index,
					frames,
				})
			},

			_ => return Err(ErrorKind::InvalidData.into()),
		})
	}
}

impl<T: Format> Frame<T> {
	pub fn new<R: Read>(from: &mut R) -> Result<Self, Error> {
		// start by reading the frame header
		let _magic = from.read_u16::<LE>()?;
		let old_chunks = from.read_u16::<LE>()? as u32;
		let duration = from.read_u16::<LE>()?;
		// skip 2 bytes
		from.read_exact(&mut [0u8; 2])?;
		let mut chunk_count = from.read_u32::<LE>()?;
		if chunk_count == 0 {
			chunk_count = old_chunks;
		}

		// then load the chunks
		let mut chunks = vec![];
		for _ in 0..chunk_count {
			let bytes = from.read_u32::<LE>()? as u64 - 4;
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

fn read_pixels<R: Read, T: Format>(from: &mut R, length: usize) -> Result<Vec<T>, Error> {
	let mut pixels = Vec::with_capacity(length);
	for _ in 0..length {
		pixels.push(T::read(from)?);
	}
	Ok(pixels)
}

impl<T: Format> Chunk<T> {
	pub fn new<R: Read>(from: &mut R) -> Result<Self, Error> {
		let chunk_type = from.read_u16::<LE>()?;

		println!("parsing chunk with type {:?}", chunk_type);
		let result = match chunk_type {
			//0x0004 => {
			//	Chunk::Palette {
					//
			//	}
			//},
			//0x0011 => {
			//	Chunk::Palette {
					//
			//	}
			//},
			0x2004 => {
				let flags = from.read_u16::<LE>()?;
				let is_group = match from.read_u16::<LE>()? { 0 => false, _ => true };
				let child_level = from.read_u16::<LE>()?;
				let width = from.read_u16::<LE>()?;
				let height = from.read_u16::<LE>()?;
				let blend = from.read_u16::<LE>()?;
				let opacity = from.read_u8()?;
				// skip 3 bytes
				from.read_exact(&mut [0u8; 3])?;
				let name = read_string(from)?;
				let cel = None;

				Chunk::Layer {
					flags,
					is_group,
					child_level,
					width,
					height,
					blend,
					opacity,
					name,
					cel,
				}
			},
			0x2005 => {
				Chunk::Cel {
					layer_index: from.read_u16::<LE>()?,
					cel: Cel {
						x: from.read_u16::<LE>()?,
						y: from.read_u16::<LE>()?,
						opacity: from.read_u8()?,
						data: match from.read_u16::<LE>()? {
							0 => {
								// skip 7 bytes
								from.read_exact(&mut [0u8; 7])?;

								// raw
								let width = from.read_u16::<LE>()?;
								let height = from.read_u16::<LE>()?;
								let data = read_pixels(from, (width*height) as _)?;
								CelData::Pixels {
									width, height, data
								}
							},
							1 => {
								// skip 7 bytes
								from.read_exact(&mut [0u8; 7])?;

								CelData::Link { 
									frame: from.read_u16::<LE>()? 
								}
							},
							2 => {
								// skip 7 bytes
								from.read_exact(&mut [0u8; 7])?;

								let width = from.read_u16::<LE>()?;
								let height = from.read_u16::<LE>()?;
								let mut decoder = Decoder::new(from)?;
								let data = read_pixels(&mut decoder, (width*height) as _)?;
								decoder.into_inner().bytes().count();
								CelData::Pixels {
									width, height, data,
								}
							},
							_ => {
								return Err(ErrorKind::InvalidData.into())
							}
						},
					},
				}
			},
			//0x2006 => {
			//	Chunk::CelExtra {
					//
			//	}
			//},
			0x2018 => {
				let count = from.read_u16::<LE>()?;
				from.read_exact(&mut [0u8; 8])?;

				Chunk::FrameTags {			
					tags: (0..count)
						.into_iter()
						.fold(Ok(vec![]), |v: Result<Vec<_>, Error>, _i| v.and_then(|mut v| {
							let from_frame = from.read_u16::<LE>()?;
							let to_frame = from.read_u16::<LE>()?;
							let loop_mode = match from.read_u8()? {
								0 => FrameLoop::Forward,
								1 => FrameLoop::Reverse,
								2 => FrameLoop::PingPong,
								_ => return Err(ErrorKind::InvalidData.into()),
							};
							from.read_exact(&mut [0u8; 8])?;
							let color = [from.read_u8()?, from.read_u8()?, from.read_u8()?];
							from.read_u8()?;
							let name = read_string(from)?;

							v.push(FrameTag { from_frame, to_frame, loop_mode, color, name });
							Ok(v)
						}))?,
				}
			},
			0x2019 => {
				let new_size = from.read_u32::<LE>()?;
				let first = from.read_u32::<LE>()?;
				let last = from.read_u32::<LE>()?;
				from.read_exact(&mut [0u8; 8])?;

				Chunk::Palette {
					new_size,
					first,
					last,
					updates: (first..=last)
						.into_iter()
						.fold(Ok(vec![]), |v: Result<Vec<_>, Error>, _| v.and_then(|mut v| {
							let flags = from.read_u16::<LE>()?;
							v.push(ColorEntry {
								color: <[u8; 4]>::read(from)?,
								name: match flags {
									0x0001 => Some(read_string(from)?),
									_ => None,
								},
							});
							Ok(v)
						}))?,
				}
			},
			0x2020 => {
				let flags = from.read_u32::<LE>()?;
				Chunk::UserData {
					text: if 1 == flags & 1 {
						Some(read_string(from)?)
					}  else {
						None
					},
					color: if 2 == flags & 2 {
						Some(<[u8; 4]>::read(from)?)
					} else {
						None
					}
				}
			},
			//0x2022 => {
			//	Chunk::Slice {
					//
			//	}
			//},
			_ => {
				from.bytes().count();
				Chunk::Unsupported
			},
		};

		Ok(result)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

    #[test]
    fn test_load_document() {
    	let bytes = include_bytes!("../character.ase").to_vec();

    	let mut cursor = ::std::io::Cursor::new(bytes);

    	Document::new(&mut cursor).unwrap();
    }
}
