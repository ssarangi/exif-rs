import struct

def read_raf_file(filepath):
    RAF_TIFF1_PTR_OFFSET = 84
    RAF_TIFF2_PTR_OFFSET = 100
    RAF_TAGS_PTR_OFFSET = 92
    with open(filepath, 'rb') as file:
        # Read the magic string
        magic = file.read(16).decode('ascii').strip()
        print("Magic String:", magic)
        
        # Read format version
        format_version = file.read(4).decode('ascii')
        print("Format Version:", format_version)
        
        # Read camera number ID
        camera_id = file.read(8).decode('ascii')
        print("Camera ID:", camera_id)
        
        # Read camera string
        camera_string = file.read(32).decode('ascii').strip('\x00')
        print("Camera String:", camera_string)
        
        # Skipping 24 bytes to reach the JPEG image offset
        file.seek(24, 1)  # relative seek from current position
        
        # Read JPEG image offset and length
        jpeg_offset = struct.unpack('>I', file.read(4))[0]
        jpeg_length = struct.unpack('>I', file.read(4))[0]
        print("JPEG Image Offset:", jpeg_offset)
        print("JPEG Image Length:", jpeg_length)
        
        # Read CFA Header Offset and Length
        cfa_header_offset = struct.unpack('>I', file.read(4))[0]
        cfa_header_length = struct.unpack('>I', file.read(4))[0]
        print("CFA Header Offset:", cfa_header_offset)
        print("CFA Header Length:", cfa_header_length)
        
        # Read CFA Offset and Length
        cfa_offset = struct.unpack('>I', file.read(4))[0]
        cfa_length = struct.unpack('>I', file.read(4))[0]
        print("CFA Offset:", cfa_offset)
        print("CFA Length:", cfa_length)




        # Optionally, go to the JPEG offset and read some JPEG data
        file.seek(jpeg_offset)
        jpeg_data = file.read(jpeg_length)  # just reading the first 10 bytes of the JPEG image
        exif_data = find_app1_exif(jpeg_data)
        if not exif_data:
            print("No EXIF data found.")
            return

        parse_exif(exif_data)

        # output = open('output.jpg', 'wb')
        # output.write(jpeg_data)

EXIF_TAGS = {
    0x010F: 'Make',
    0x0110: 'Model',
    0x0112: 'Orientation',
    0x011A: 'XResolution',
    0x011B: 'YResolution',
    0x0128: 'ResolutionUnit',
    0x0131: 'Software',
    0x0132: 'DateTime',
    0x0213: 'YCbCrPositioning',
    0x8769: 'ExifOffset',
    0x829A: 'ExposureTime',
    0x829D: 'FNumber',
    0x8827: 'ISOSpeedRatings',
    0x9003: 'DateTimeOriginal',
    0x9004: 'DateTimeDigitized',
    0x9201: 'ShutterSpeedValue',
    0x9202: 'ApertureValue',
    0x9204: 'ExposureBiasValue',
    0x9207: 'MeteringMode',
    0x9209: 'Flash',
    0x9214: 'SubjectArea',
    0x927C: 'MakerNote',
    0x9286: 'UserComment',
    0xA001: 'ColorSpace',
    0xA002: 'ExifImageWidth',
    0xA003: 'ExifImageHeight',
    0xA005: 'InteroperabilityOffset',
    0xA20E: 'FocalPlaneXResolution',
    0xA20F: 'FocalPlaneYResolution',
    0xA210: 'FocalPlaneResolutionUnit',
    0xA217: 'SensingMethod',
    0xA300: 'FileSource',
    0xA301: 'SceneType',
}

def find_app1_exif(data):
    """ Finds the APP1 EXIF marker and extracts the EXIF block. """
    index = 0
    while index + 4 < len(data):
        if data[index:index+2] == b'\xFF\xE1':
            length = struct.unpack('>H', data[index+2:index+4])[0]
            if data[index+4:index+10] == b'Exif\0\0':
                return data[index+10:index+2+length]
        index += 1
    return None

def parse_exif(data):
    """ Parses the EXIF data from a JPEG's EXIF block. """
    if data[:2] in (b'II', b'MM'):
        endian = '<' if data[:2] == b'II' else '>'
    else:
        print("Invalid TIFF data.")
        return

    # Check the TIFF header
    magic, = struct.unpack(endian + 'H', data[2:4])
    if magic != 42:
        print("Invalid TIFF magic number.")
        return

    # Get the offset to the first IFD
    first_ifd_offset, = struct.unpack(endian + 'L', data[4:8])
    offset = first_ifd_offset + 6  # Plus six because TIFF header starts after 'Exif\0\0'

    while offset and offset < len(data) - 2:
        num_tags, = struct.unpack(endian + 'H', data[offset:offset+2])
        offset += 2
        print(f"Reading {num_tags} tags at offset {offset}")

        for _ in range(num_tags):
            if offset > len(data) - 12:
                print("Offset out of bounds for data size")
                return  # Safety check for buffer overflow
            tag, typ, count, value_offset = struct.unpack(endian + 'HHLL', data[offset:offset+12])
            tag_name = EXIF_TAGS.get(tag, f"Unknown tag 0x{tag:04X}")
            print(f"Tag {tag_name} ({tag}) at offset {offset}")

            # Move to the next tag
            offset += 12

        # Move to the next IFD
        if offset + 4 > len(data):
            break
        next_ifd_offset, = struct.unpack(endian + 'L', data[offset:offset+4])
        if next_ifd_offset == 0:
            break
        offset = next_ifd_offset

def main():
    filepath = "/Users/satyajits/Pictures/TestFolder/_DSF5533.RAF"
    read_raf_file(filepath)

if __name__ == "__main__":
    main()
