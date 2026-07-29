#version 300 es

// ============================================================================
// points.vert
// ============================================================================
//
// 이 파일은 Point Cloud의 "점 하나"가 화면 어디에 찍힐지 계산하는 shader.
//
// OpenGL 렌더링 파이프라인에서 Vertex Shader 단계에 해당.
// Rust에서 넘긴 point buffer 하나하나에 대해 실행됨.
//
// Rust 쪽 point 데이터 포맷:
//
// [x, y, z, value, x, y, z, value, ...]
//
// 즉 점 하나는 float 4개:
//
// x     : 3D X 좌표
// y     : 3D Y 좌표
// z     : 3D Z 좌표
// value : 색상 계산에 사용할 값
//
// 이 shader는 직접 색상을 칠하지 않음.
// 위치 계산과 point size 설정만 하고,
// value는 fragment shader로 넘겨줌.

// ============================================================================
// [INPUT] Rust VBO에서 넘어오는 per-vertex 데이터
// ============================================================================
//
// layout(location = 0)
//
// Rust 쪽에서:
//
// gl::VertexAttribPointer(0, 3, ...)
//
// 로 연결한 attribute.
// 점의 3D 위치를 의미함.
layout(location = 0) in vec3 aPos;

// layout(location = 1)
//
// Rust 쪽에서:
//
// gl::VertexAttribPointer(1, 1, ...)
//
// 로 연결한 attribute.
// 점의 색상 계산에 사용할 scalar 값.
// 예: 높이, 거리, intensity, depth, z value 등.
layout(location = 1) in float aValue;

// ============================================================================
// [UNIFORM] 모든 점에 공통으로 적용되는 값
// ============================================================================
//
// uniform은 vertex마다 다른 값이 아니라,
// draw call 전체에 동일하게 적용되는 외부 입력값.
//
// Rust에서 gl::Uniform... 함수로 값을 넣어줌.

// Model-View-Projection matrix.
//
// 3D 좌표를 화면에 표시 가능한 clip space 좌표로 변환하는 행렬.
//
// 역할:
//
// Model      : 물체 자체 변환
// View       : camera 위치/방향 반영
// Projection : 원근 투영
//
// 최종 변환:
//
// gl_Position = uMVP * vec4(aPos, 1.0)
uniform mat4 uMVP;

// 화면에 보이는 점 크기.
//
// Rust/Dart 쪽 UI에서 point size를 조절하면
// 이 값이 uniform으로 shader에 들어옴.
//
// gl_PointSize에 이 값을 넣으면 GL_POINTS로 그리는 점 크기가 바뀜.
uniform float uPointSize;

// ============================================================================
// [OUTPUT] Fragment Shader로 넘길 값
// ============================================================================
//
// Vertex Shader에서 계산하거나 받은 값을 Fragment Shader로 전달할 때 out 사용.
//
// 여기서는 aValue를 그대로 넘김.
// Fragment Shader는 이 vValue를 기준으로 색상을 계산함.
out float vValue;

// ============================================================================
// [MAIN]
// ============================================================================
//
// vertex 하나마다 한 번씩 실행.
// point cloud가 100,000개면 이 main도 100,000번 실행됨.
void main() {
    // ------------------------------------------------------------------------
    // 1. 3D 좌표를 OpenGL clip space로 변환
    // ------------------------------------------------------------------------
    //
    // aPos는 원래 3D world 좌표.
    // vec4(aPos, 1.0)은 vec3를 행렬 곱셈 가능한 homogeneous coordinate로 확장.
    //
    // gl_Position은 OpenGL 내장 출력 변수.
    // Vertex Shader는 반드시 gl_Position을 써야 함.
    //
    // 이 값이 최종적으로 화면상의 위치 계산에 사용됨.
    gl_Position = uMVP * vec4(aPos, 1.0f);

    // ------------------------------------------------------------------------
    // 2. 점 크기 지정
    // ------------------------------------------------------------------------
    //
    // GL_POINTS로 렌더링할 때 점 하나의 화면 크기를 지정.
    //
    // Rust 쪽에서 gl::Enable(gl::PROGRAM_POINT_SIZE)를 켜야
    // shader의 gl_PointSize가 실제로 반영됨.
    gl_PointSize = uPointSize;

    // ------------------------------------------------------------------------
    // 3. 색상 계산용 값 전달
    // ------------------------------------------------------------------------
    //
    // Vertex Shader는 위치 계산 담당.
    // 실제 색상은 Fragment Shader에서 계산하므로 value를 넘겨줌.
    vValue = aValue;
}