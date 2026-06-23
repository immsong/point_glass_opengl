import 'package:flutter/material.dart';

import 'package:vector_math/vector_math.dart' as vm;

import 'point_glass_opengl_label.dart';

/// 3D 공간의 원점(0,0,0)에 X, Y, Z 축을 표시하기 위한 설정 모델
class PointGlassOpenGLAxis {
  final bool enable;
  final double length;
  final double lineWidth;

  final Color colorX;
  final Color colorY;
  final Color colorZ;

  final bool labelEnable;
  final PointGlassOpenGLLabel labelX;
  final PointGlassOpenGLLabel labelY;
  final PointGlassOpenGLLabel labelZ;

  PointGlassOpenGLAxis({
    this.enable = true,
    this.length = 1.0,
    this.lineWidth = 2.0,
    this.colorX = const Color.fromARGB(255, 255, 50, 50),
    this.colorY = const Color.fromARGB(255, 50, 255, 50),
    this.colorZ = const Color.fromARGB(255, 50, 100, 255),
    this.labelEnable = true,
    PointGlassOpenGLLabel? labelX,
    PointGlassOpenGLLabel? labelY,
    PointGlassOpenGLLabel? labelZ,
  }) : labelX =
           labelX ??
           PointGlassOpenGLLabel(
             position: vm.Vector3(length / 2.0, lineWidth * 0.02, 0.0),
             text: 'X',
             style: TextStyle(
               color: Colors.white70,
               fontSize: 12,
               fontWeight: FontWeight.bold,
             ),
           ),
       labelY =
           labelY ??
           PointGlassOpenGLLabel(
             position: vm.Vector3(lineWidth * 0.02, length / 2.0, 0.0),
             text: 'Y',
             style: TextStyle(
               color: Colors.white70,
               fontSize: 12,
               fontWeight: FontWeight.bold,
             ),
           ),
       labelZ =
           labelZ ??
           PointGlassOpenGLLabel(
             position: vm.Vector3(
               lineWidth * 0.02,
               lineWidth * 0.02,
               length / 2.0,
             ),
             text: 'Z',
             style: TextStyle(
               color: Colors.white70,
               fontSize: 12,
               fontWeight: FontWeight.bold,
             ),
           );
}
