use crate::error::Result;

use allegro::*;
use allegro_primitives::*;

#[derive(Clone)]
pub struct Mesh
{
	vtxs: Vec<NormVertex>,
	idxs: Vec<i32>,
	material: Option<String>,
}

#[derive(Clone)]
pub struct MultiMesh
{
	meshes: Vec<Mesh>,
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
								v: uv[1],
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
					meshes.push(Mesh {
						vtxs: vtxs,
						idxs: idxs,
						material: prim.material().name().map(|x| x.into()),
					});
				}
			}
		}
		Ok(Self { meshes: meshes })
	}

	pub fn draw(&self, prim: &PrimitivesAddon)
	{
		for mesh in self.meshes.iter()
		{
			prim.draw_indexed_prim(
				&mesh.vtxs[..],
				Option::<&Bitmap>::None,
				//mesh.material					.as_ref()					.and_then(|m| materials.get(m.as_str())),
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
struct NormVertex
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
