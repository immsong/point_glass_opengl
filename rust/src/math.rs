// ============================================================================
// Math
// ============================================================================
//
// OpenGL 렌더링과 좌표 변환에서 공통으로 사용하는 순수 수학 함수.
// Renderer나 OpenGL context 상태에는 의존하지 않음.

// 아래 함수들은 OpenGL 자체 함수가 아니라,
// 우리가 직접 3D camera/projection 계산을 하기 위한 수학 함수들.
//
// OpenGL shader에는 최종적으로 uMVP matrix 하나만 넘김.
// 그 uMVP는 아래 흐름으로 만들어짐.
//
// Model matrix      : 물체 자체 회전/이동
// View matrix       : camera 위치/방향
// Projection matrix : 3D 공간을 2D 화면처럼 보이게 투영
//
// 최종:
// MVP = Projection * View * Model
//
// shader에서는 보통 이렇게 사용:
//
// gl_Position = uMVP * vec4(position, 1.0);

// camera 위치, target 위치, up 방향을 이용해서 view matrix 생성
//
// eye    : camera 위치
// target : camera가 바라보는 지점
// up     : 화면 위쪽 방향
//
// 예:
// eye    = [0, 3, 8]
// target = [0, 0, 0]
// up     = [0, 1, 0]
//
// 이 함수 결과는 "world 좌표를 camera 기준 좌표로 바꾸는 matrix"라고 보면 됨.
pub(crate) fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    // f = forward vector
    //
    // camera가 바라보는 방향.
    // target - eye 로 계산.
    //
    // normalize 해서 길이가 1인 방향 벡터로 만듦.
    let f = {
        let r = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];

        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();

        [r[0] / len, r[1] / len, r[2] / len]
    };

    // s = side/right vector
    //
    // forward 방향과 up 방향의 cross product.
    // 즉 camera 기준 오른쪽 방향.
    //
    // cross(f, up)
    let s = {
        let r = [
            f[1] * up[2] - f[2] * up[1],
            f[2] * up[0] - f[0] * up[2],
            f[0] * up[1] - f[1] * up[0],
        ];

        let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();

        [r[0] / len, r[1] / len, r[2] / len]
    };

    // u = real up vector
    //
    // 입력으로 받은 up은 완전히 직교하지 않을 수 있음.
    // 그래서 s와 f를 기준으로 다시 up 방향을 계산.
    let u = [
        s[1] * f[2] - s[2] * f[1],
        s[2] * f[0] - s[0] * f[2],
        s[0] * f[1] - s[1] * f[0],
    ];

    // OpenGL column-major 기준 matrix.
    //
    // s: camera 오른쪽 축
    // u: camera 위쪽 축
    // -f: camera 뒤쪽 축
    //
    // 마지막 줄 쪽에는 camera 위치 보정값이 들어감.
    [
        s[0],
        u[0],
        -f[0],
        0.0,
        s[1],
        u[1],
        -f[1],
        0.0,
        s[2],
        u[2],
        -f[2],
        0.0,
        -(s[0] * eye[0] + s[1] * eye[1] + s[2] * eye[2]),
        -(u[0] * eye[0] + u[1] * eye[1] + u[2] * eye[2]),
        f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2],
        1.0,
    ]
}

// perspective projection matrix 생성
//
// fovy   : 세로 시야각, radian 단위
// aspect : width / height
// near   : camera에 가장 가까운 렌더링 거리
// far    : camera에서 가장 먼 렌더링 거리
//
// 이 matrix가 있어야 멀리 있는 점은 작게 보이고,
// 가까운 점은 크게 보이는 원근감이 생김.
pub(crate) fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    // focal scale 같은 값.
    // fovy가 작을수록 zoom-in처럼 보임.
    let g = 1.0 / (fovy * 0.5).tan();

    [
        g / aspect,
        0.0,
        0.0,
        0.0,
        0.0,
        g,
        0.0,
        0.0,
        0.0,
        0.0,
        (far + near) / (near - far),
        -1.0,
        0.0,
        0.0,
        (2.0 * far * near) / (near - far),
        0.0,
    ]
}

// 4x4 matrix 곱셈
//
// OpenGL에서 주로 사용하는 column-major 형태 기준.
// a * b 결과를 반환.
//
// MVP 만들 때 사용:
// vp  = projection * view
// mvp = vp * model
pub(crate) fn multiply_matrices(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut res = [0.0f32; 16];

    for col in 0..4 {
        for row in 0..4 {
            res[col * 4 + row] = a[row] * b[col * 4]
                + a[4 + row] * b[col * 4 + 1]
                + a[8 + row] * b[col * 4 + 2]
                + a[12 + row] * b[col * 4 + 3];
        }
    }

    res
}

// 4x4 matrix 역행렬 계산
//
// 입력과 출력은 기존 OpenGL 코드와 동일한 column-major 형식을 사용.
//
// 화면의 NDC 좌표를 다시 3D 좌표로 되돌리려면
// MVP matrix의 역행렬이 필요함.
//
// 처리 방식:
//
// 1. column-major 배열을 행 단위 augmented matrix로 변환
// 2. Gauss-Jordan elimination으로 역행렬 계산
// 3. 결과를 다시 column-major 배열로 변환
//
// 역행렬을 만들 수 없는 singular matrix인 경우 None 반환.
pub(crate) fn invert_matrix(matrix: [f32; 16]) -> Option<[f32; 16]> {
    // 왼쪽에는 원본 matrix,
    // 오른쪽에는 identity matrix를 배치.
    //
    // [M | I]
    let mut augmented = [[0.0f32; 8]; 4];

    for row in 0..4 {
        for col in 0..4 {
            // 입력 matrix는 column-major 형식
            augmented[row][col] = matrix[col * 4 + row];
        }

        augmented[row][4 + row] = 1.0;
    }

    // Gauss-Jordan elimination 수행
    for pivot_col in 0..4 {
        // 현재 column에서 절댓값이 가장 큰 값을 pivot으로 선택.
        //
        // 작은 값을 pivot으로 사용하면 부동소수점 오차가 커질 수 있으므로
        // partial pivoting을 적용.
        let mut pivot_row = pivot_col;

        for row in (pivot_col + 1)..4 {
            if augmented[row][pivot_col].abs() > augmented[pivot_row][pivot_col].abs() {
                pivot_row = row;
            }
        }

        // pivot이 거의 0이면 역행렬을 만들 수 없는 matrix.
        if augmented[pivot_row][pivot_col].abs() < 1.0e-8 {
            return None;
        }

        // 필요한 경우 현재 row와 pivot row 교환
        if pivot_row != pivot_col {
            augmented.swap(pivot_row, pivot_col);
        }

        // pivot 값을 1로 정규화
        let pivot = augmented[pivot_col][pivot_col];

        for col in 0..8 {
            augmented[pivot_col][col] /= pivot;
        }

        // 다른 row의 현재 column 값을 0으로 제거
        for row in 0..4 {
            if row == pivot_col {
                continue;
            }

            let factor = augmented[row][pivot_col];

            for col in 0..8 {
                augmented[row][col] -= factor * augmented[pivot_col][col];
            }
        }
    }

    // 오른쪽에 만들어진 역행렬을
    // OpenGL column-major 배열로 다시 변환
    let mut inverse = [0.0f32; 16];

    for row in 0..4 {
        for col in 0..4 {
            inverse[col * 4 + row] = augmented[row][4 + col];
        }
    }

    Some(inverse)
}

// 4x4 matrix와 4차원 vector 곱셈
//
// 기존 shader와 project_3d_to_screen_batch()에서 사용하는 것과
// 동일한 OpenGL column-major 계산 방식을 사용.
//
// result = matrix * vector
pub(crate) fn transform_vec4(matrix: [f32; 16], vector: [f32; 4]) -> [f32; 4] {
    [
        matrix[0] * vector[0]
            + matrix[4] * vector[1]
            + matrix[8] * vector[2]
            + matrix[12] * vector[3],
        matrix[1] * vector[0]
            + matrix[5] * vector[1]
            + matrix[9] * vector[2]
            + matrix[13] * vector[3],
        matrix[2] * vector[0]
            + matrix[6] * vector[1]
            + matrix[10] * vector[2]
            + matrix[14] * vector[3],
        matrix[3] * vector[0]
            + matrix[7] * vector[1]
            + matrix[11] * vector[2]
            + matrix[15] * vector[3],
    ]
}
