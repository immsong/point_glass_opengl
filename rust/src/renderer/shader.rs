use std::ffi::{CStr, CString};
use std::ptr;

// ============================================================================
// Shader
// ============================================================================

//
// shader는 OS가 아니라 현재 OpenGL context 종류에 맞춰 선택 필요.
//
// OpenGL ES 3.0+ context      -> GLSL ES 3.00 (#version 300 es)
// Desktop OpenGL 3.3+ context -> GLSL 3.30 Core (#version 330 core)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShaderProfile {
    Gles300,
    Gl330,
}

pub(crate) struct ShaderSources {
    pub(crate) points_vert: &'static str,
    pub(crate) points_frag: &'static str,
    pub(crate) gizmos_vert: &'static str,
    pub(crate) gizmos_frag: &'static str,
}

const POINTS_VERT_GLES300: &str = include_str!("../../shaders/gles300/points.vert");
const POINTS_FRAG_GLES300: &str = include_str!("../../shaders/gles300/points.frag");
const GIZMOS_VERT_GLES300: &str = include_str!("../../shaders/gles300/gizmos.vert");
const GIZMOS_FRAG_GLES300: &str = include_str!("../../shaders/gles300/gizmos.frag");

const POINTS_VERT_GL330: &str = include_str!("../../shaders/gl330/points.vert");
const POINTS_FRAG_GL330: &str = include_str!("../../shaders/gl330/points.frag");
const GIZMOS_VERT_GL330: &str = include_str!("../../shaders/gl330/gizmos.vert");
const GIZMOS_FRAG_GL330: &str = include_str!("../../shaders/gl330/gizmos.frag");

pub(crate) fn shader_sources(profile: ShaderProfile) -> ShaderSources {
    match profile {
        ShaderProfile::Gles300 => ShaderSources {
            points_vert: POINTS_VERT_GLES300,
            points_frag: POINTS_FRAG_GLES300,
            gizmos_vert: GIZMOS_VERT_GLES300,
            gizmos_frag: GIZMOS_FRAG_GLES300,
        },
        ShaderProfile::Gl330 => ShaderSources {
            points_vert: POINTS_VERT_GL330,
            points_frag: POINTS_FRAG_GL330,
            gizmos_vert: GIZMOS_VERT_GL330,
            gizmos_frag: GIZMOS_FRAG_GL330,
        },
    }
}

fn parse_version_number(value: &str) -> Option<(u32, u32)> {
    value.split_whitespace().find_map(|token| {
        if !token.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return None;
        }

        let mut parts = token.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor_text = parts.next()?;
        let minor_digits: String = minor_text
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let minor = minor_digits.parse::<u32>().ok()?;

        Some((major, minor))
    })
}

unsafe fn read_gl_string(name: u32, label: &str) -> Result<String, String> {
    let value = unsafe { gl::GetString(name) };

    if value.is_null() {
        return Err(format!("glGetString({label}) returned null"));
    }

    Ok(unsafe { CStr::from_ptr(value.cast()) }
        .to_string_lossy()
        .into_owned())
}

// 현재 연결된 context의 API와 버전을 검사하여 사용할 shader profile 반환.
// OS가 아니라 glGetString(GL_VERSION)의 실제 결과를 기준으로 판단.
pub(crate) unsafe fn detect_shader_profile() -> Result<ShaderProfile, String> {
    let gl_version = unsafe { read_gl_string(gl::VERSION, "GL_VERSION") }?;
    let glsl_version = unsafe { read_gl_string(gl::SHADING_LANGUAGE_VERSION, "GLSL_VERSION") }?;

    println!("OpenGL version: {gl_version}");
    println!("GLSL version: {glsl_version}");

    let gl_number = parse_version_number(&gl_version)
        .ok_or_else(|| format!("Unable to parse OpenGL version: {gl_version}"))?;
    let glsl_number = parse_version_number(&glsl_version)
        .ok_or_else(|| format!("Unable to parse GLSL version: {glsl_version}"))?;

    if gl_version.starts_with("OpenGL ES") {
        if gl_number < (3, 0) {
            return Err(format!(
                "OpenGL ES 3.0 or later is required, current version: {gl_version}"
            ));
        }

        if !glsl_version.contains("GLSL ES") || glsl_number < (3, 0) {
            return Err(format!(
                "GLSL ES 3.00 or later is required, current version: {glsl_version}"
            ));
        }

        return Ok(ShaderProfile::Gles300);
    }

    if gl_number < (3, 3) {
        return Err(format!(
            "Desktop OpenGL 3.3 or later is required, current version: {gl_version}"
        ));
    }

    if glsl_version.contains("GLSL ES") || glsl_number < (3, 30) {
        return Err(format!(
            "Desktop GLSL 3.30 or later is required, current version: {glsl_version}"
        ));
    }

    Ok(ShaderProfile::Gl330)
}

// shader source를 정규화하고 선택된 profile의 #version인지 확인.
// shader 파일에 버전이 빠지거나 서로 뒤바뀐 경우 임의 보정하지 않고 오류 처리.
fn clean_shader_source(src: &str, profile: ShaderProfile) -> Result<String, String> {
    let normalized = src
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    let normalized = normalized.trim_start();
    let first_line = normalized.lines().next().unwrap_or_default().trim();

    let expected_version = match profile {
        ShaderProfile::Gles300 => "#version 300 es",
        ShaderProfile::Gl330 => "#version 330 core",
    };

    if first_line != expected_version {
        return Err(format!(
            "Shader profile mismatch: expected '{expected_version}', but starts with '{first_line}'"
        ));
    }

    Ok(normalized.to_string())
}

unsafe fn compile_shader(shader_type: u32, source: &str) -> Result<u32, String> {
    let shader = unsafe { gl::CreateShader(shader_type) };

    if shader == 0 {
        return Err("glCreateShader returned 0".to_string());
    }

    let c_source = match CString::new(source) {
        Ok(source) => source,
        Err(_) => {
            unsafe { gl::DeleteShader(shader) };
            return Err("Shader source contains an interior null byte".to_string());
        }
    };

    unsafe {
        gl::ShaderSource(shader, 1, &c_source.as_ptr(), ptr::null());
        gl::CompileShader(shader);
    }

    let mut success = 0;
    unsafe { gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success) };

    if success == 0 {
        let mut length = 0;
        unsafe { gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut length) };

        let mut log = vec![0u8; length.max(1) as usize];
        unsafe {
            gl::GetShaderInfoLog(shader, length, ptr::null_mut(), log.as_mut_ptr().cast());
            gl::DeleteShader(shader);
        }

        let log = String::from_utf8_lossy(&log)
            .trim_matches(char::from(0))
            .to_string();
        let preview = source.lines().take(12).collect::<Vec<_>>().join("\n");

        return Err(format!(
            "Shader compile failed:\n{log}\n--- source preview ---\n{preview}"
        ));
    }

    Ok(shader)
}

pub(crate) unsafe fn create_program(
    v_src: &str,
    f_src: &str,
    profile: ShaderProfile,
) -> Result<u32, String> {
    let vertex_source = clean_shader_source(v_src, profile)?;
    let fragment_source = clean_shader_source(f_src, profile)?;

    let vertex_shader = unsafe { compile_shader(gl::VERTEX_SHADER, &vertex_source) }
        .map_err(|error| format!("Vertex {error}"))?;

    let fragment_shader = match unsafe { compile_shader(gl::FRAGMENT_SHADER, &fragment_source) } {
        Ok(shader) => shader,
        Err(error) => {
            unsafe { gl::DeleteShader(vertex_shader) };
            return Err(format!("Fragment {error}"));
        }
    };

    let program = unsafe { gl::CreateProgram() };
    if program == 0 {
        unsafe {
            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);
        }
        return Err("glCreateProgram returned 0".to_string());
    }

    unsafe {
        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);
    }

    let mut success = 0;
    unsafe { gl::GetProgramiv(program, gl::LINK_STATUS, &mut success) };

    unsafe {
        gl::DeleteShader(vertex_shader);
        gl::DeleteShader(fragment_shader);
    }

    if success == 0 {
        let mut length = 0;
        unsafe { gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut length) };

        let mut log = vec![0u8; length.max(1) as usize];
        unsafe {
            gl::GetProgramInfoLog(program, length, ptr::null_mut(), log.as_mut_ptr().cast());
            gl::DeleteProgram(program);
        }

        let log = String::from_utf8_lossy(&log)
            .trim_matches(char::from(0))
            .to_string();

        return Err(format!("Program link failed:\n{log}"));
    }

    Ok(program)
}
