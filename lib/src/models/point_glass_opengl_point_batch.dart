import 'point_glass_opengl_points_color_mode.dart';

/// Point cloud의 특정 구간에 적용할 Batch별 표시 설정.
///
/// [first], [count]는 Float32List 인덱스가 아닌 Point 개수 기준.
/// 각 Point는 `[x, y, z, value]` 형식으로 4개의 Float 값을 가짐.
///
/// Batch는 [first] 기준으로 정렬되어야 하며, 서로 겹치거나
/// Point cloud 데이터 범위를 초과할 수 없음.
/// Batch 사이의 빈 구간은 렌더링하지 않음.
class PointGlassOpenGLPointBatch {
  final int first;
  final int count;
  final double pointSize;
  final double opacity;
  final double valueMin;
  final double valueMax;
  final PointGlassOpenGLPointsColorMode colorMode;

  const PointGlassOpenGLPointBatch({
    required this.first,
    required this.count,
    this.pointSize = 3.0,
    this.opacity = 1.0,
    this.valueMin = -2.0,
    this.valueMax = 5.0,
    this.colorMode = PointGlassOpenGLPointsColorMode.viridis,
  });
}
