import ghidra.app.script.GhidraScript;
public class PiranhaPhysicsDescriptorEvidence extends GhidraScript {
  private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
  private void dump(long base,String name)throws Exception{
    println("\n=== "+name+String.format(" %08X ===",base));
    for(int off=-0x20;off<=0x40;off+=4)println(String.format("off=%d %08X",off,ptr(base+off)));
  }
  @Override public void run()throws Exception{
    dump(0x005DFDF4L,"piranha-first-update descriptor ptr");
    dump(0x005DFE18L,"known projectile physics vtable");
    dump(0x005DFC3CL,"candidate character descriptor");
  }
}