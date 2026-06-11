import 'dart:typed_data';
import '../models/point_glass_opengl_points.dart';

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
}
