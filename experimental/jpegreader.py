import struct

# Common EXIF tags
EXIF_TAGS = {
    0x0100: 'ImageWidth',
    0x0101: 'ImageHeight',
    0x010f: 'Make',
    0x0110: 'Model',
    0x0112: 'Orientation',
    0x011a: 'XResolution',
    0x011b: 'YResolution',
    0x013b: 'Artist',
    0x013e: 'WhitePoint',
    0x013f: 'PrimaryChromaticities',
    0x0128: 'ResolutionUnit',
    0x0131: 'Software',
    0x0132: 'DateTime',
    0x0211: 'YCbCrCoefficients',
    0x0213: 'YCbCrPositioning',
    0x8769: 'ExifOffset',
    0x8298: 'Copyright',
    0x8769: 'Exif Offset',
    0x829a: 'ExposureTime',
    0x829d: 'FNumber',
    0x8827: 'ISOSpeedRatings',
    0x9003: 'DateTimeOriginal',
    0x9004: 'DateTimeDigitized',
    0x9201: 'ShutterSpeedValue',
    0x9202: 'ApertureValue',
    0x9204: 'ExposureBiasValue',
    0x9206: 'SubjectDistance',
    0x9207: 'MeteringMode',
    0x9209: 'Flash',
    0x927c: 'MakerNote',
    0x9286: 'UserComment',
    0xa001: 'ColorSpace',
    0xa002: 'ExifImageWidth',
    0xa003: 'ExifImageHeight',
    0xa005: 'InteroperabilityOffset',
    0xa430: 'CameraOwnerName',
    0xa431: 'SerialNumber',
    0xa432: 'LensInfo',
    0xa433: 'LensMake',
    0xa434: 'LensModel',
    0xa435: 'LensSerialNumber',
    0xc4a5: 'PrintIM',
}

def parse_exif(data):
    start = data.find(b'\xff\xe1')
    if start == -1:
        raise ValueError("No APP1 header found.")
    
    # length = int.from_bytes(data[start+2:start+4], byteorder='big')
    
    start += 4
    # end = start + length
    # exif_data = data[start:end]
    
    if data[start: start + 6] != b'Exif\0\0':
        raise ValueError("Invalid EXIF data.")
    
    endian = data[start + 6:start + 8]
    endian_symbol = '<' if endian == b'II' else '>'

    offset_to_ifd = int.from_bytes(data[start + 10:start + 14], byteorder='little' if endian_symbol == '<' else 'big')
    offset = start + 6 + offset_to_ifd
    
    number_of_tags, = struct.unpack(f'{endian_symbol}H', data[offset:offset+2])
    offset += 2

    file_offset = 12

    for _ in range(number_of_tags):
        tag, type, count, value_offset = struct.unpack(f'{endian_symbol}HHII', data[offset:offset+12])
        tag_name = EXIF_TAGS.get(tag, f'Unknown tag 0x{tag:04X}')

        print(tag_name, type, value_offset, file_offset + value_offset, count)
        # count += 1

        value = None
        if type == 1:  # Byte
            value = data[file_offset + value_offset: file_offset + value_offset + count]
        elif type == 2:  # ASCII
            value = data[file_offset + value_offset:file_offset + value_offset + count].decode('ascii')
        elif type == 3:  # Short
            value_format = f'{endian_symbol}{count}H'
            value = struct.unpack(value_format, data[file_offset + value_offset:file_offset + value_offset + 2 * count])
        elif type == 4:  # Long
            value_format = f'{endian_symbol}{count}I'
            value = struct.unpack(value_format, data[file_offset + value_offset:file_offset + value_offset + 4 * count])
        elif type == 5:  # Rational
            value_format = f'{endian_symbol}{count * 2}I'
            rational_values = struct.unpack(value_format, data[file_offset + value_offset:file_offset + value_offset + 8 * count])
            value = [(rational_values[i], rational_values[i+1]) for i in range(0, len(rational_values), 2)]
        
        print(f"Tag {tag_name} (0x{tag:04X}): {value}")
        offset += 12

def read_exif_from_jpeg(filepath):
    with open(filepath, 'rb') as file:
        data = file.read()
    
    parse_exif(data)

# Example usage
read_exif_from_jpeg('output.jpg')
