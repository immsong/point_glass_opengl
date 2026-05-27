# point_glass_opengl

`point_glass_opengl`은 Flutter에서 대용량 3D point cloud를 더 자연스럽고 빠르게 렌더링하기 위한 OpenGL 기반 패키지입니다. 
기존 `point_glass`의 Dart Canvas 렌더러와 별개로, Windows, Linux, Android 환경에서 GPU 가속을 활용한 렌더링을 제공합니다.

## 핵심 목표
* 대용량 point cloud를 GPU 기반으로 렌더링
* 확대/축소 시 공간이 휘어져 보이는 문제 해결 및 극단 확대 후에도 3D 깊이감 유지
* Flutter UI는 유지하되, 렌더링 core는 Rust/OpenGL로 분리

## 지원 플랫폼 (초기 버전 기준)
* ✅ **Android** (OpenGL ES + EGL - 예정)
* ✅ **Windows** (OpenGL / WGL or EGL - 예정)
* ✅ **Linux** (OpenGL / EGL or GLX - 예정)
> *macOS, iOS, Web은 지원하지 않습니다.*