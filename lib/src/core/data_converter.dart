import 'dart:typed_data';

import 'package:point_glass_opengl/src/models/point_glass_opengl_points.dart';
import 'package:point_glass_opengl/src/models/point_glass_opengl_grid.dart';

class DataConverter {
  /// PointGlassOpenGLPoints 그룹 배열을 OpenGL용 Float32List로 변환
  /// 포맷: [X, Y, Z, R, G, B, A, Size]
  static Float32List convertPointsGroup(List<PointGlassOpenGLPoints> groups) {
    final List<double> result = [];

    for (final group in groups) {
      if (!group.enable) {
        continue;
      }

      final size = group.pointSize;
      for (final pt in group.points) {
        result.addAll([
          pt.x,
          pt.y,
          pt.z,
          group.color.r,
          group.color.g,
          group.color.b,
          group.color.a,
          size,
        ]);
      }
    }

    return Float32List.fromList(result);
  }

  /// Grid 설정값을 OpenGL Lines용 데이터로 변환
  /// 포맷: [X, Y, Z, R, G, B, A, Thickness]
  static Float32List convertGrid(PointGlassOpenGLGrid grid) {
    if (!grid.enable) return Float32List(0);

    final List<double> result = [];

    // gridSize가 20, gridStep이 1이면 count는 20칸.
    // 절반(half)은 10이 되어, -10부터 +10까지 선을 긋습니다.
    final int count = (grid.gridSize / grid.gridStep).round();
    final double half = grid.gridSize / 2.0;

    final r = grid.color.r;
    final g = grid.color.g;
    final b = grid.color.b;
    final a = grid.color.a;
    final w = grid.lineWidth;

    for (int i = 0; i <= count; i++) {
      double t = -half + (i * grid.gridStep);

      // 가로선 (X축에 평행하게 쭉 긋고, Y축(t) 위치를 이동)
      result.addAll([-half, t, 0.0, r, g, b, a, w]); // 시작점
      result.addAll([half, t, 0.0, r, g, b, a, w]); // 끝점

      // 세로선 (Y축에 평행하게 쭉 긋고, X축(t) 위치를 이동)
      result.addAll([t, -half, 0.0, r, g, b, a, w]); // 시작점
      result.addAll([t, half, 0.0, r, g, b, a, w]); // 끝점
    }

    return Float32List.fromList(result);
  }
}
