export function h264DecoderConfig() {
  return {
    codec: 'avc1.42E01F',
    optimizeForLatency: true,
    avc: { format: 'annexb' },
  };
}

export function isH264KeyPacket(packet: Uint8Array) {
  for (let index = 0; index + 3 < packet.length; index += 1) {
    if (
      index + 4 < packet.length &&
      packet[index] === 0 &&
      packet[index + 1] === 0 &&
      packet[index + 2] === 0 &&
      packet[index + 3] === 1
    ) {
      const nalType = packet[index + 4] & 0x1f;
      if (nalType === 5 || nalType === 7) {
        return true;
      }
    } else if (packet[index] === 0 && packet[index + 1] === 0 && packet[index + 2] === 1) {
      const nalType = packet[index + 3] & 0x1f;
      if (nalType === 5 || nalType === 7) {
        return true;
      }
    }
  }

  return false;
}

export function readH264Packets(buffer: Uint8Array) {
  const packets: Uint8Array[] = [];
  let offset = 0;

  while (buffer.length - offset >= 4) {
    const length = new DataView(buffer.buffer, buffer.byteOffset + offset, 4).getUint32(0, false);
    if (buffer.length - offset < 4 + length) {
      break;
    }
    packets.push(buffer.slice(offset + 4, offset + 4 + length));
    offset += 4 + length;
  }

  return {
    packets,
    remaining: buffer.slice(offset),
  };
}
