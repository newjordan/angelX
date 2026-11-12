struct Geometry { position: vec4f, axis0: vec4f, axis1: vec4f, axis2: vec4f }
struct View { r0:vec4f, r1:vec4f, r2:vec4f, params:vec4f, options:vec4f, eye:vec4f, sh:vec4u }
@group(0) @binding(0) var<uniform> camera:View;
@group(0) @binding(1) var<storage,read> geometry:array<Geometry>;
@group(0) @binding(2) var<storage,read> order:array<u32>;
@group(0) @binding(3) var<storage,read> harmonics:array<f32>;
@group(0) @binding(4) var<storage,read> harmonics_second:array<f32>;
fn coefficient(index:u32)->f32 {let split=camera.sh.y;if index<split{return harmonics[index];}return harmonics_second[index-split];}
fn rotate(p:vec3f)->vec3f {return vec3f(dot(camera.r0.xyz,p),dot(camera.r1.xyz,p),dot(camera.r2.xyz,p));}
fn inverse_rotate(p:vec3f)->vec3f{return camera.r0.xyz*p.x+camera.r1.xyz*p.y+camera.r2.xyz*p.z;}
// Covariance factor A=R diag(s): projected covariance is (J A)(J A)^T.
// Its determinant is |cross(Jx A, Jy A)|², with no catastrophic ac-b²
// subtraction. Thin positive axes are never squared/rounded away on upload.
fn project_factor(g:Geometry,v:vec3f)->vec3f {
 return vec3f(dot(v,g.axis0.xyz),dot(v,g.axis1.xyz),dot(v,g.axis2.xyz));
}
struct Vertex { @builtin(position) position:vec4f, @location(0) local:vec2f, @location(1) color:vec4f }
@vertex fn vertex(@builtin(vertex_index) v:u32,@builtin(instance_index) instance:u32)->Vertex {
 var out:Vertex;out.position=vec4f(2.,2.,2.,1.);out.local=vec2f(4.);out.color=vec4f(0.);
 let id=order[instance];let g=geometry[id];let p=rotate(g.position.xyz)-vec3f(camera.eye.xy,0.);let d=camera.params.z-p.z;
 if d<0.05 || g.position.w<0.0039 {return out;}
 let width=camera.params.x;let height=camera.params.y;let f=min(width,height)*camera.params.w;
 let gx=inverse_rotate(vec3f(f/d,0.,f*p.x/(d*d)));
 let gy=inverse_rotate(vec3f(0.,-f/d,-f*p.y/(d*d)));
 let u=project_factor(g,gx);let v_axis=project_factor(g,gy);
 let raw_aa=dot(u,u);let bb=dot(u,v_axis);let raw_cc=dot(v_axis,v_axis);
 let area=cross(u,v_axis);let det_raw=dot(area,area);
 let aa=raw_aa+0.3;let cc=raw_cc+0.3;
 let det_filtered=det_raw+0.3*(raw_aa+raw_cc)+0.09;
 // Convolution grows the footprint, not total energy. Without this determinant
 // ratio, subpixel splats become unnaturally opaque/bright and shimmer on zoom.
 var opacity=g.position.w;
 if camera.options.z>0.5 {
   opacity*=sqrt(clamp(det_raw/det_filtered,0.,1.));
 }
 if opacity<0.0039215686 {return out;}
 var support=3.;
 if camera.options.z>0.5 {support=sqrt(2.*log(opacity*255.));}
 let mid=0.5*(aa+cc);let delta=sqrt(max(0.,0.25*(aa-cc)*(aa-cc)+bb*bb));
 let lambda1=max(0.3,mid+delta);
 // Product of eigenvalues is det: this division retains the minor axis even
 // when mid-delta would lose it. The pixel filter bounds lambda2 >= 0.3.
 let lambda2=max(0.3,det_filtered/lambda1);
 var eigen=vec2f(1.,0.);if abs(bb)>0.00001 {eigen=normalize(vec2f(bb,lambda1-aa));}else if cc>aa {eigen=vec2f(0.,1.);}
 let major=eigen*min(sqrt(lambda1)*support,2048.);let minor=vec2f(-eigen.y,eigen.x)*min(sqrt(lambda2)*support,2048.);
 let corners=array<vec2f,6>(vec2f(-1.,-1.),vec2f(1.,-1.),vec2f(-1.,1.),vec2f(-1.,1.),vec2f(1.,-1.),vec2f(1.,1.));
 let centre=vec2f(width*.5+f*p.x/d,height*.5-f*p.y/d);
 let extent=abs(major)+abs(minor);
 if any(centre+extent<vec2f(0.)) || any(centre-extent>vec2f(width,height)) {return out;}
 let corner=corners[v];let pixel=centre+major*corner.x+minor*corner.y;
 out.position=vec4f(pixel.x/width*2.-1.,1.-pixel.y/height*2.,0.,1.);out.local=corner*support;
 let coefficients=camera.sh.x;let offset=id*coefficients;
 let original_dir=normalize((g.position.xyz-inverse_rotate(vec3f(camera.eye.xy,camera.params.z)))*vec3f(1.,-1.,-1.));
 let x=original_dir.x;let y=original_dir.y;let z=original_dir.z;
 var basis:array<f32,16>;basis[0]=0.2820947918;
 basis[1]=-0.4886025119*y;basis[2]=0.4886025119*z;basis[3]=-0.4886025119*x;
 basis[4]=1.0925484306*x*y;basis[5]=-1.0925484306*y*z;basis[6]=0.3153915653*(2.*z*z-x*x-y*y);basis[7]=-1.0925484306*x*z;basis[8]=0.5462742153*(x*x-y*y);
 basis[9]=-0.5900435899*y*(3.*x*x-y*y);basis[10]=2.8906114426*x*y*z;basis[11]=-0.4570457995*y*(4.*z*z-x*x-y*y);basis[12]=0.3731763326*z*(2.*z*z-3.*x*x-3.*y*y);basis[13]=-0.4570457995*x*(4.*z*z-x*x-y*y);basis[14]=1.4453057213*z*(x*x-y*y);basis[15]=-0.5900435899*x*(x*x-3.*y*y);
 var color=vec3f(0.5);for(var i=0u;i<coefficients;i++){let j=(offset+i)*3u;color+=basis[i]*vec3f(coefficient(j),coefficient(j+1u),coefficient(j+2u));}
 color=max(color,vec3f(0.))*exp2(camera.options.y);
 out.color=vec4f(color,opacity);return out;
}
@fragment fn fragment(in:Vertex)->@location(0) vec4f {
 let r2=dot(in.local,in.local);if camera.options.z<0.5 && r2>9. {discard;}
 let alpha=min(0.99,exp(-0.5*r2)*in.color.a);if alpha<0.0039215686 {discard;}
 return vec4f(in.color.rgb*alpha,alpha);
}
