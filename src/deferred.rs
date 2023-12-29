use crate::error::Result;

pub struct GBuffer
{
	pub frame_buffer: u32,
	pub position_tex: u32,
	pub normal_tex: u32,
	pub albedo_tex: u32,
	pub depth_render_buffer: u32,
}

impl GBuffer
{
	pub fn new(buffer_width: i32, buffer_height: i32) -> Result<Self>
	{
		let mut frame_buffer = 0;
		let mut position_tex = 0;
		let mut normal_tex = 0;
		let mut albedo_tex = 0;
		let mut depth_render_buffer = 0;

		unsafe {
			gl::GenFramebuffers(1, &mut frame_buffer);
			gl::BindFramebuffer(gl::FRAMEBUFFER, frame_buffer);

			gl::GenTextures(1, &mut position_tex);
			gl::BindTexture(gl::TEXTURE_2D, position_tex);
			gl::TexImage2D(
				gl::TEXTURE_2D,
				0,
				gl::RGBA16F as i32,
				buffer_width,
				buffer_height,
				0,
				gl::RGBA,
				gl::FLOAT,
				std::ptr::null(),
			);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
			gl::FramebufferTexture2D(
				gl::FRAMEBUFFER,
				gl::COLOR_ATTACHMENT0,
				gl::TEXTURE_2D,
				position_tex,
				0,
			);

			gl::GenTextures(1, &mut normal_tex);
			gl::BindTexture(gl::TEXTURE_2D, normal_tex);
			gl::TexImage2D(
				gl::TEXTURE_2D,
				0,
				gl::RGBA16F as i32,
				buffer_width,
				buffer_height,
				0,
				gl::RGBA,
				gl::FLOAT,
				std::ptr::null(),
			);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
			gl::FramebufferTexture2D(
				gl::FRAMEBUFFER,
				gl::COLOR_ATTACHMENT1,
				gl::TEXTURE_2D,
				normal_tex,
				0,
			);

			gl::GenTextures(1, &mut albedo_tex);
			gl::BindTexture(gl::TEXTURE_2D, albedo_tex);
			gl::TexImage2D(
				gl::TEXTURE_2D,
				0,
				gl::RGBA as i32,
				buffer_width,
				buffer_height,
				0,
				gl::RGBA,
				gl::UNSIGNED_BYTE,
				std::ptr::null(),
			);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
			gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
			gl::FramebufferTexture2D(
				gl::FRAMEBUFFER,
				gl::COLOR_ATTACHMENT2,
				gl::TEXTURE_2D,
				albedo_tex,
				0,
			);

			let attachments = [
				gl::COLOR_ATTACHMENT0,
				gl::COLOR_ATTACHMENT1,
				gl::COLOR_ATTACHMENT2,
			];
			gl::DrawBuffers(attachments.len() as i32, attachments.as_ptr());
			gl::GenRenderbuffers(1, &mut depth_render_buffer);
			gl::BindRenderbuffer(gl::RENDERBUFFER, depth_render_buffer);
			gl::RenderbufferStorage(
				gl::RENDERBUFFER,
				gl::DEPTH_COMPONENT16,
				buffer_width,
				buffer_height,
			);
			gl::FramebufferRenderbuffer(
				gl::FRAMEBUFFER,
				gl::DEPTH_ATTACHMENT,
				gl::RENDERBUFFER,
				depth_render_buffer,
			);
			if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE
			{
				return Err("Framebuffer not complete".to_string())?;
			}
			gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
		}
		Ok(Self {
			frame_buffer: frame_buffer,
			position_tex: position_tex,
			normal_tex: normal_tex,
			albedo_tex: albedo_tex,
			depth_render_buffer: depth_render_buffer,
		})
	}

	pub fn bind(&self)
	{
		unsafe {
			gl::BindFramebuffer(gl::FRAMEBUFFER, self.frame_buffer);
			let attachments = [
				gl::COLOR_ATTACHMENT0,
				gl::COLOR_ATTACHMENT1,
				gl::COLOR_ATTACHMENT2,
			];
			gl::DrawBuffers(attachments.len() as i32, attachments.as_ptr());
		}
	}
}

impl Drop for GBuffer
{
	fn drop(&mut self)
	{
		unsafe {
			gl::DeleteTextures(1, &self.position_tex);
			gl::DeleteTextures(1, &self.normal_tex);
			gl::DeleteTextures(1, &self.albedo_tex);
			gl::DeleteRenderbuffers(1, &self.depth_render_buffer);
			gl::DeleteFramebuffers(1, &self.frame_buffer);
		}
	}
}
