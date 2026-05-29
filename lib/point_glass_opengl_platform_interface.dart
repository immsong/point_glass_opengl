import 'package:plugin_platform_interface/plugin_platform_interface.dart';

import 'point_glass_opengl_method_channel.dart';

abstract class PointGlassOpenglPlatform extends PlatformInterface {
  /// Constructs a PointGlassOpenglPlatform.
  PointGlassOpenglPlatform() : super(token: _token);

  static final Object _token = Object();

  static PointGlassOpenglPlatform _instance = MethodChannelPointGlassOpengl();

  /// The default instance of [PointGlassOpenglPlatform] to use.
  ///
  /// Defaults to [MethodChannelPointGlassOpengl].
  static PointGlassOpenglPlatform get instance => _instance;

  /// Platform-specific implementations should set this with their own
  /// platform-specific class that extends [PointGlassOpenglPlatform] when
  /// they register themselves.
  static set instance(PointGlassOpenglPlatform instance) {
    PlatformInterface.verifyToken(instance, _token);
    _instance = instance;
  }

  Future<String?> getPlatformVersion() {
    throw UnimplementedError('platformVersion() has not been implemented.');
  }
}
