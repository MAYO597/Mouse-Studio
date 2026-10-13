"""Rebuild the MX Anywhere 3S from the four supplied product-image references.
blender --background --factory-startup --python scripts/build_mouse_model.py
The top silhouette is sampled from the PNG alpha outline. Crown/heel sections,
seams, sculpted grip channels and control placement follow all four views.
"""
import bpy
import json
import math
from pathlib import Path
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/models'
OUT.mkdir(parents=True, exist_ok=True)
REF = json.loads((ROOT / 'scripts/mouse-reference.json').read_text())
SCALE = 0.1005 / 3.10
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
parts = []


def material(name, color, metallic=0, roughness=.52):
    mat = bpy.data.materials.new(name)
    mat.diffuse_color = (*color, 1)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get('Principled BSDF')
    bsdf.inputs['Base Color'].default_value = (*color, 1)
    bsdf.inputs['Metallic'].default_value = metallic
    bsdf.inputs['Roughness'].default_value = roughness
    bsdf.inputs['Specular IOR Level'].default_value = .28
    return mat

shell = material('Graphite upper shell', (.030, .033, .038))
rubber = material('Sculpted silicone grips', (.027, .030, .034), roughness=.52)
base = material('Graphite lower rim', (.043, .048, .055), roughness=.48)
black = material('Recesses and seams', (.005, .007, .009), roughness=.48)
spine = material('Satin center insert', (.049, .053, .061), roughness=.4)
metal = material('Brushed steel wheel', (.48, .52, .57), metallic=.9, roughness=.25)
feet = material('PTFE feet', (.24, .26, .28), roughness=.7)
indicator = material('Reference green indicator', (.018, .95, .006), roughness=.3)
ink = material('Printed logi mark', (.014, .017, .021), roughness=.65)


def coord(p):
    return p[0]*SCALE, -p[2]*SCALE, p[1]*SCALE


def mesh_object(name, vertices, faces, mat):
    triangles=[]
    cut_wheel=name in ['Shell left','Shell right','Center panel','Center panel seam','Wheel recess']
    lookup={tuple(round(c,9) for c in p):i for i,p in enumerate(vertices)}
    def emit(poly):
        ids=[]
        for p in poly:
            key=tuple(round(c,9) for c in p)
            if key not in lookup:
                lookup[key]=len(vertices)
                vertices.append(tuple(p))
            ids.append(lookup[key])
        for k in range(1,len(ids)-1):
            tri=(ids[0],ids[k],ids[k+1])
            a,b,c=(Vector(vertices[i]) for i in tri)
            if (b-a).cross(c-a).length_squared>1e-15: triangles.append(tri)
    center_z=(340-128)*3.1/1142-1.55
    planes=[lambda p:p.x+.117,lambda p:.117-p.x,lambda p:p.z-center_z+.238,lambda p:center_z+.238-p.z]
    for face in faces:
        for k in range(1,len(face)-1):
            poly=[Vector(vertices[i]) for i in (face[0],face[k],face[k+1])]
            if not cut_wheel:
                emit(poly)
                continue
            # Exact polygon subtraction keeps a smooth rectangular opening;
            # deleting triangles by their centers leaves a sawtooth rim.
            for plane in planes:
                inside=[];outside=[]
                if not poly: break
                prev=poly[-1]; dp=plane(prev)
                for cur in poly:
                    dc=plane(cur)
                    if (dp>=0)!=(dc>=0):
                        hit=prev+(cur-prev)*(dp/(dp-dc))
                        inside.append(hit);outside.append(hit)
                    (inside if dc>=0 else outside).append(cur)
                    prev=cur;dp=dc
                emit(outside)
                poly=inside
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([coord(p) for p in vertices], [], triangles)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    obj.data.materials.append(mat)
    for polygon in mesh.polygons:
        polygon.use_smooth = True
    parts.append(obj)
    return obj


def grid(name, nu, nv, sample, mat, flip=False):
    vertices = [sample(i/nu,j/nv) for i in range(nu+1) for j in range(nv+1)]
    faces = []
    for i in range(nu):
        for j in range(nv):
            a=i*(nv+1)+j
            f=(a,a+nv+1,a+nv+2,a+1)
            faces.append(f[::-1] if flip else f)
    return mesh_object(name, vertices, faces, mat)


def interp(table, z):
    # Monotone cubic interpolation avoids bumps between measured sections.
    if z <= table[0][0]: return table[0][1]
    if z >= table[-1][0]: return table[-1][1]
    i=next(i for i in range(len(table)-1) if table[i+1][0]>=z)
    x0,y0=table[i]; x1,y1=table[i+1]
    slope=(y1-y0)/(x1-x0)
    def tangent(k):
        if k==0: return (table[1][1]-table[0][1])/(table[1][0]-table[0][0])
        if k==len(table)-1: return (table[-1][1]-table[-2][1])/(table[-1][0]-table[-2][0])
        a=(table[k][1]-table[k-1][1])/(table[k][0]-table[k-1][0])
        b=(table[k+1][1]-table[k][1])/(table[k+1][0]-table[k][0])
        return 2*a*b/(a+b) if a*b>0 else 0
    t=(z-x0)/(x1-x0)
    return (2*t**3-3*t*t+1)*y0+(t**3-2*t*t+t)*(x1-x0)*tangent(i)+(-2*t**3+3*t*t)*y1+(t**3-t*t)*(x1-x0)*tangent(i+1)

CROWN=[(-1.55,-.20),(-1.5,-.02),(-1.4,.045),(-1.2,.18),(-.9,.34),(-.6,.48),(-.3,.585),(0,.646),(.3,.665),(.6,.645),(.9,.59),(1.15,.49),(1.35,.36),(1.48,.28),(1.55,.20)]
BOTTOM=[(-1.55,-.20),(-1.5,-.31),(-1.35,-.42),(-1.1,-.43),(1.1,-.43),(1.3,-.39),(1.45,-.14),(1.55,.20)]
SEAM=[(-1.55,.76),(-1.2,.80),(-.6,.865),(0,.88),(.5,.88),(.8,.87),(1.05,.86),(1.3,.84),(1.55,.82)]

def width(z): return max(0,interp(REF['width_profile'],z))
def crown(z): return interp(CROWN,z)
def bottom(z): return interp(BOTTOM,z)
def seam(z): return interp(SEAM,z)
def xf(f):
    if f>=.42:
        return max(0,1-((f-.42)/.58)**2)**.125
    return 1-.17*((.42-f)/.42)**2

def surface(z,f,sign=1,offset=0):
    return sign*(width(z)*xf(f)+offset), bottom(z)+(crown(z)-bottom(z))*f,z

def top(x,z):
    ratio=min(1,abs(x)/max(.000001,width(z)))
    f=.42+.58*math.sqrt(max(0,1-ratio**8))
    return bottom(z)+(crown(z)-bottom(z))*f

def shell_surface(z,v,sign):
    x=sign*width(z)*xf(seam(z))*v
    return x,top(x,z),z

# The crown, shoulder and rolled heel are continuous curved surfaces.
for name,sign in [('Shell left',-1),('Shell right',1)]:
    grid(name,144,36,lambda u,v,s=sign: shell_surface(-1.55+3.1*u,v,s),shell,flip=sign<0)
    grid(name+' rounded edge',144,4,lambda u,v,s=sign: surface(-1.55+3.1*u,seam(-1.55+3.1*u)-.018*v,s,.002*math.sin(v*math.pi)),shell,flip=sign<0)
    grid(name+' seam',144,2,lambda u,v,s=sign: surface(-1.55+3.1*u,seam(-1.55+3.1*u)-.018-.01*v,s),black,flip=sign<0)
    grid('Base' if sign<0 else 'Base right',144,12,lambda u,v,s=sign: surface(-1.55+3.1*u,.19*(1-v),s),base,flip=sign<0)
    grid('Side housing '+str(sign),144,16,lambda u,v,s=sign: surface(-1.55+3.1*u,.19+(seam(-1.55+3.1*u)-.219)*(1-v),s,-.040*math.sin(math.pi*v)**.5),base,flip=sign<0)

grid('Underside',144,16,lambda u,v: ((2*v-1)*width(-1.55+3.1*u)*.83,bottom(-1.55+3.1*u)-.001,-1.55+3.1*u),base,flip=True)

# The five sweeping channels are depressions in one smooth grip surface.
# They converge near the nose, fan out under the buttons and curve up at the heel.
def grip(u,v,sign):
    z=-1.36+2.68*u
    low=.19+.50*u**8
    high=low+(seam(z)-.04-low)*u**.35
    close=max(0,(u-.94)/.06)
    low=low+(high-low)*close**2
    f=low+(high-low)*v
    offset=.005*math.sin(math.pi*u)**.6*math.sin(math.pi*v)**.4
    fade=math.sin(math.pi*u)**1.6*math.sin(math.pi*v)**.6
    for k in range(5):
        line=.07+(k+.6)*.137+.12*u**3
        offset-=.022*math.exp(-((v-line)/.065)**2)*fade
    return surface(z,f,sign,offset)
for name,sign in [('Left grip',-1),('Right grip',1)]:
    grid(name,144,80,lambda u,v,s=sign: grip(u,v,s),rubber,flip=sign>0)


def capsule(name,cx,cz,w,length,lift,mat,height_fn=top,radius=None,dome=0):
    r=w/2 if radius is None else radius
    def sample(u,v):
        if u<.22:
            z=cz-length/2+r*(1-math.cos(u/.22*math.pi/2))
        elif u>.78:
            z=cz+length/2-r+r*math.sin((u-.78)/.22*math.pi/2)
        else:
            z=cz-length/2+r+(u-.22)/.56*(length-2*r)
        end=max(0,abs(z-cz)-(length/2-r))
        half=w/2-r+math.sqrt(max(0,r*r-end*end))
        x=cx+(2*v-1)*half
        y=height_fn(x,z)+lift+dome*math.sin(math.pi*u)**.4*math.sin(math.pi*v)**.4
        return x,y,z
    return grid(name,64,12,sample,mat,flip=height_fn!=top)

# Positions are measured from the top PNG: front row 128, rear row 1270.
def photo_z(row): return (row-128)*3.1/1142-1.55
grid('Front button split',32,2,lambda u,v:((v-.5)*.014,top((v-.5)*.014,-1.535+.182*u)+.003,-1.535+.182*u),black)
capsule('Center panel seam',0,photo_z(526),.412,1.790,.004,black)
capsule('Center panel',0,photo_z(526),.387,1.765,.009,spine)
capsule('Wheel recess',0,photo_z(340),.252,.510,.014,black,radius=.035)
capsule('Wheel well',0,photo_z(340),.254,.515,-.20,black,radius=.035)
capsule('Mode button recess',0,photo_z(529),.155,.338,.015,black)
capsule('Mode button',0,photo_z(529),.129,.310,.023,shell,dome=.012)
capsule('Status LED lens',0,photo_z(632),.054,.080,.017,indicator,dome=.002)

wheel_z=photo_z(340)
wheel_y=top(0,wheel_z)-.100

def wheel(u,v):
    theta=v*math.tau
    x=(u-.5)*.201
    r=.235+(.0045 if int(v*160)%2==0 else 0)
    if abs(x)>.091: r-=.009
    return x,wheel_y+math.cos(theta)*r,wheel_z+math.sin(theta)*r

grid('Scroll wheel',12,320,wheel,metal,flip=True)
for s in [-1,1]:
    grid('Wheel end cap '+str(s),8,96,lambda u,v,s=s:(s*.101,wheel_y+u*.226*math.cos(s*v*math.tau),wheel_z+u*.226*math.sin(s*v*math.tau)),metal)
grid('Wheel center channel',1,160,lambda u,v:((u-.5)*.026,wheel_y+.242*math.cos(v*math.tau),wheel_z+.242*math.sin(v*math.tau)),black,flip=True)

# Rounded thumb keys nest closely beneath the shoulder seam.
def thumb(name,start,end,inset,mat):
    def sample(u,v):
        z=start+(end-start)*u
        rounded=.65+.35*math.sin(math.pi*u)**.20
        high=seam(z)-.046
        low=high-.105*rounded
        f=low+(high-low)*v
        bulge=.007*math.sin(math.pi*u)*math.sin(math.pi*v)
        return surface(z,f,-1,inset+bulge)
    return grid(name,40,12,sample,mat)
for name,a,b in [('Forward button',-.43,.045),('Back button',.065,.54)]:
    thumb(name+' recess',a-.011,b+.011,.005,black)
    thumb(name,a,b,.015,shell)

# Printed logo traced from the supplied top image; each cell follows the crown.
vertices=[]; faces=[]
for x,y in REF['logo_pixels']:
    start=len(vertices)
    for dx,dy in [(0,0),(0,1),(1,1),(1,0)]:
        px=(x+dx-800)*3.1/1142
        pz=photo_z(y+dy)
        vertices.append((px,top(px,pz)+.010,pz))
    faces.append((start,start+1,start+2,start+3))
mesh_object('Printed logi wordmark',vertices,faces,ink)

# The charging socket sits low in the nose, mostly concealed in three-quarter views.
def port(name,w,h,lift,mat):
    def sample(u,v):
        a=v*math.tau
        x=w*math.copysign(abs(math.cos(a))**.6,math.cos(a))*u
        y=-.27+h*math.copysign(abs(math.sin(a))**.7,math.sin(a))*u
        z=-1.515+lift+.022*(abs(x)/w)**2
        return x,y,z
    return grid(name,4,64,sample,mat)
port('USB-C socket rim',.142,.045,0,base)
port('USB-C socket',.127,.034,-.002,black)
port('USB-C tongue',.085,.007,-.003,base)

bottom_surface=lambda x,z:bottom(z)-.008
capsule('Optical sensor recess',0,0,.30,.40,0,black,bottom_surface)
capsule('Optical sensor lens',0,0,.15,.19,-.003,black,bottom_surface)
for z in [-1.05,1.02]:
    for x in [-.43,.43]:
        capsule(f'Glide foot {x} {z}',x,z,.21,.36,-.005,feet,bottom_surface)
capsule('Power switch recess',.43,.1,.12,.30,0,black,bottom_surface)
capsule('Power switch',.43,.055,.085,.14,-.005,shell,bottom_surface)
capsule('Easy-Switch button',0,.54,.18,.18,-.005,shell,bottom_surface)
# Export the same mesh used by the editor to a compact dependency-free app format.
depsgraph = bpy.context.evaluated_depsgraph_get()
hotspots=[surface(.30,seam(.30)-.095,-1,.026),surface(-.20,seam(-.20)-.095,-1,.026),(0,wheel_y+.248,wheel_z),(-.43,top(-.43,-.9)+.015,-.9),(.43,top(.43,-.9)+.015,-.9)]
asset = {'version': 1, 'description': 'Reconstructed from the four supplied MX Anywhere 3S PNG references', 'hotspots': hotspots, 'parts': []}
for obj in parts:
    evaluated = obj.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    mesh.calc_loop_triangles()
    vertices, normals, triangles = [], [], []
    lookup = {}
    for tri in mesh.loop_triangles:
        face = []
        for loop_index in tri.loops:
            loop = mesh.loops[loop_index]
            p = obj.matrix_world @ mesh.vertices[loop.vertex_index].co
            n = obj.matrix_world.to_3x3() @ mesh.corner_normals[loop_index].vector
            n.normalize()
            pos = [round(p.x/SCALE, 6), round(p.z/SCALE, 6), round(-p.y/SCALE, 6)]
            normal = [round(n.x, 6), round(n.z, 6), round(-n.y, 6)]
            key = tuple(pos + normal)
            if key not in lookup:
                lookup[key] = len(vertices)
                vertices.append(pos)
                normals.append(normal)
            face.append(lookup[key])
        a, b, c = (Vector(vertices[i]) for i in face)
        if (b-a).cross(c-a).length_squared > 1e-15:
            triangles.append(face)
    mat = obj.data.materials[0]
    asset['parts'].append({'name': obj.name, 'color': list(mat.diffuse_color)[:3], 'metallic': mat.node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value, 'vertices': vertices, 'normals': normals, 'triangles': triangles})
    evaluated.to_mesh_clear()
(OUT / 'mx-anywhere-3s.mesh.json').write_text(json.dumps(asset, separators=(',', ':')))

# GLB contains only product objects, in meters, with the same named components.
bpy.ops.object.select_all(action='DESELECT')
for obj in parts:
    obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(OUT / 'mx-anywhere-3s.glb'), export_format='GLB', use_selection=True)

# Four review cameras correspond to the four supplied PNGs.
scene=bpy.context.scene
scene.unit_settings.system='METRIC'
scene.render.engine='CYCLES'
scene.cycles.samples=48
scene.cycles.use_denoising=True
scene.render.resolution_x=1600
scene.render.resolution_y=1374
scene.render.resolution_percentage=100
scene.render.image_settings.file_format='PNG'
scene.render.film_transparent=True
scene.world.color=(.09,.09,.09)
scene.view_settings.view_transform='AgX'
scene.view_settings.look='AgX - Medium High Contrast'

def point_at(obj,at):
    obj.rotation_euler=(Vector(at)-obj.location).to_track_quat('-Z','Y').to_euler()

cameras=[]
for name,location,scale in [('front',(-.30,.18,.14),.125),('top',(0,0,.4),.14),('side',(-.4,0,.018),.125),('back',(-.25,-.18,.16),.13)]:
    bpy.ops.object.camera_add(location=location)
    cam=bpy.context.object
    cam.name='Reference '+name
    cam.data.type='ORTHO'
    cam.data.ortho_scale=scale
    point_at(cam,(0,0,.002))
    cameras.append((name,cam))
for name,location,energy,size in [('Key softbox',(-.16,.06,.24),.65,.18),('Fill softbox',(.14,.06,.16),.30,.20),('Rear rim',(0,-.18,.18),.90,.17),('Grip fill',(-.22,0,.045),.28,.22)]:
    bpy.ops.object.light_add(type='AREA',location=location)
    light=bpy.context.object
    light.name=name
    light.data.energy=energy
    light.data.shape='DISK'
    light.data.size=size
    point_at(light,(0,0,0))
scene.camera=cameras[0][1]
bpy.ops.object.select_all(action='DESELECT')
for obj in parts: obj.select_set(True)
bpy.context.view_layer.objects.active=parts[0]
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.region_3d.view_distance=.20
            area.spaces.active.region_3d.view_location=(0,0,0)
bpy.context.preferences.filepaths.save_version=0
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'mx-anywhere-3s.blend'))
for name,cam in cameras:
    scene.camera=cam
    scene.render.filepath=str(OUT/'mx-anywhere-3s-preview.png') if name=='front' else str(OUT/('mx-anywhere-3s-'+name+'.png'))
    bpy.ops.render.render(write_still=True)
print(f'Exported {len(parts)} components, {sum(len(p["triangles"]) for p in asset["parts"])} triangles')
