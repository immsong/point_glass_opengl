import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'point_glass_opengl_platform_interface.dart';

/// An implementation of [PointGlassOpenglPlatform] that uses method channels.
class MethodChannelPointGlassOpengl extends PointGlassOpenglPlatform {
  /// The method channel used to interact with the native platform.
  @visibleForTesting
  final methodChannel = const MethodChannel('point_glass_opengl');

  @override
  Future<String?> getPlatformVersion() async {
    final version = await methodChannel.invokeMethod<String>(
      'getPlatformVersion',
    );
    return version;
  }
}
