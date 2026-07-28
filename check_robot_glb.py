import struct, json

f = open('crates/rgpui-3d/examples/3d/robot.glb', 'rb')
# skip header
f.read(12)
# read JSON chunk
cl = struct.unpack('<I', f.read(4))[0]
ct = struct.unpack('<I', f.read(4))[0]
d = f.read(cl).decode('utf-8').rstrip('\x00')
data = json.loads(d)

print('=== SKINS ===')
for i, skin in enumerate(data.get('skins', [])):
    joints = skin.get('joints', [])
    print('Skin %d: skeleton=%s num_joints=%d' % (i, skin.get('skeleton'), len(joints)))
    print('  joints: %s' % joints[:10])

print()
print('=== NODES WITH MESH ===')
for i, node in enumerate(data.get('nodes', [])):
    name = node.get('name', '')
    if 'mesh' in node:
        skin = node.get('skin')
        ch = node.get('children', [])
        print('  Node %d: name="%s" mesh=%s skin=%s children=%s' % (i, name, node['mesh'], skin, ch))

print()
print('=== ROOT NODES ===')
all_children = set()
for node in data.get('nodes', []):
    for c in node.get('children', []):
        all_children.add(c)
for i in range(len(data.get('nodes', []))):
    if i not in all_children:
        node = data['nodes'][i]
        print('  Root Node %d: name="%s" mesh=%s skin=%s' % (i, node.get('name',''), node.get('mesh'), node.get('skin')))