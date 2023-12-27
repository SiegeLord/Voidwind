use crate::error::Result;
use crate::utils;
use serde_derive::{Deserialize, Serialize};

use allegro::*;
use allegro_primitives::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MaterialDesc
{
	pub texture: String,
}

#[derive(Debug, Clone)]
pub struct Material
{
	pub name: String,
	pub desc: MaterialDesc,
}

#[derive(Clone, Debug)]
pub struct Mesh
{
	pub vtxs: Vec<NormVertex>,
	pub idxs: Vec<i32>,
	pub material: Option<Material>,
}

#[derive(Clone, Debug)]
pub struct MultiMesh
{
	pub meshes: Vec<Mesh>,
}

impl MultiMesh
{
	pub fn load(gltf_file: &str) -> Result<Self>
	{
		let (document, buffers, _) = gltf::import(gltf_file)?;
		let mut meshes = vec![];
		for node in document.nodes()
		{
			if let Some(mesh) = node.mesh()
			{
				for prim in mesh.primitives()
				{
					let mut vtxs = vec![];
					let mut idxs = vec![];
					let reader = prim.reader(|buffer| Some(&buffers[buffer.index()]));
					if let (
						Some(pos_iter),
						Some(gltf::mesh::util::ReadTexCoords::F32(uv_iter)),
						Some(normal_iter),
					) = (
						reader.read_positions(),
						reader.read_tex_coords(0),
						reader.read_normals(),
					)
					{
						for ((pos, uv), normal) in pos_iter.zip(uv_iter).zip(normal_iter)
						{
							vtxs.push(NormVertex {
								x: pos[0],
								y: pos[1],
								z: pos[2],
								u: uv[0],
								v: 1. - uv[1],
								nx: normal[0],
								ny: normal[1],
								nz: normal[2],
								color: Color::from_rgb_f(1., 1., 1.),
							});
						}
					}

					if let Some(iter) = reader.read_indices()
					{
						for idx in iter.into_u32()
						{
							idxs.push(idx as i32)
						}
					}

					let material = prim
						.material()
						.name()
						.map(|name| {
							(
								name.to_string(),
								utils::load_config(&format!("data/{}.cfg", name)),
							)
						})
						.map_or(Ok(None), |(name, desc)| {
							desc.map(|desc| {
								Some(Material {
									name: name,
									desc: desc,
								})
							})
						})?;
					meshes.push(Mesh {
						vtxs: vtxs,
						idxs: idxs,
						material: material,
					});
				}
			}
		}
		Ok(Self { meshes: meshes })
	}

	pub fn draw<'l, T: Fn(&str, &str) -> Option<&'l Bitmap>>(&self, prim: &PrimitivesAddon, bitmap_fn: T)
	{
		for mesh in self.meshes.iter()
		{
			prim.draw_indexed_prim(
				&mesh.vtxs[..],
				mesh.material
					.as_ref()
					.and_then(|m| bitmap_fn(&m.name, &m.desc.texture)),
				&mesh.idxs[..],
				0,
				mesh.idxs.len() as u32,
				PrimType::TriangleList,
			);
		}
	}
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct NormVertex
{
	x: f32,
	y: f32,
	z: f32,
	u: f32,
	v: f32,
	nx: f32,
	ny: f32,
	nz: f32,
	color: Color,
}

unsafe impl VertexType for NormVertex
{
	fn get_decl(prim: &PrimitivesAddon) -> VertexDecl
	{
		fn make_builder() -> std::result::Result<VertexDeclBuilder, ()>
		{
			VertexDeclBuilder::new(std::mem::size_of::<NormVertex>())
				.pos(
					VertexAttrStorage::F32_3,
					memoffset::offset_of!(NormVertex, x),
				)?
				.uv(
					VertexAttrStorage::F32_2,
					memoffset::offset_of!(NormVertex, u),
				)?
				.color(memoffset::offset_of!(NormVertex, color))?
				.user_attr(
					VertexAttrStorage::F32_3,
					memoffset::offset_of!(NormVertex, nx),
				)
		}

		VertexDecl::from_builder(prim, &make_builder().unwrap())
	}
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct WaterVertex
{
	pub x: f32,
	pub y: f32,
	pub z: f32,
}

unsafe impl VertexType for WaterVertex
{
	fn get_decl(prim: &PrimitivesAddon) -> VertexDecl
	{
		fn make_builder() -> std::result::Result<VertexDeclBuilder, ()>
		{
			VertexDeclBuilder::new(std::mem::size_of::<WaterVertex>()).pos(
				VertexAttrStorage::F32_3,
				memoffset::offset_of!(WaterVertex, x),
			)
		}

		VertexDecl::from_builder(prim, &make_builder().unwrap())
	}
}
