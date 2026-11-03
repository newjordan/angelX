exports.sounds = {
  fairies: { asset: asset('bloom.wav').id, level: 0.3 },
  spark: { asset: asset('spark.wav').id, level: 0.35 },
  bloom: { asset: asset('bloom.wav').id, level: 0.4 }
}
exports.banks = {
  crystal: exports.sounds,
  warm: { fairies: { asset: asset('spark.wav').id, level: 0.25 }, spark: { asset: asset('bloom.wav').id, level: 0.3 }, bloom: { asset: asset('spark.wav').id, level: 0.25 } },
  quiet: { fairies: { asset: asset('bloom.wav').id, level: 0 }, spark: { asset: asset('spark.wav').id, level: 0 }, bloom: { asset: asset('bloom.wav').id, level: 0 } }
}
