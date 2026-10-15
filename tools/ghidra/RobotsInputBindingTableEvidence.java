import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class RobotsInputBindingTableEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 long u32(long a)throws Exception{byte[] b=new byte[4];currentProgram.getMemory().getBytes(toAddr(a),b);return ((long)b[0]&255)|(((long)b[1]&255)<<8)|(((long)b[2]&255)<<16)|(((long)b[3]&255)<<24);}
 void dec(long x)throws Exception{Function f=getFunctionContaining(toAddr(x));println(String.format("\n=== DEC 0x%08X ===",x));if(f==null){println("NO_FUNCTION");return;}println(f.getName()+" @ "+f.getEntryPoint());DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 public void run()throws Exception{
  d.openProgram(currentProgram);
  long[] bases={0x005e1758L,0x005e1698L};
  for(long base:bases){
    println(String.format("\n=== DEFAULTS 0x%08X ===",base));
    for(int i=0;i<0x30;i++) println(String.format("[%02X] 0x%08X",i,u32(base+i*4)));
  }
  println("\n=== OVERRIDE RECORDS DAT_005E14C8 ===");
  long base=0x005e14c8L;
  for(int off=0,rec=0;off<0x1cc;off+=0x14,rec++){
    long a=base+off;
    println(String.format("rec%02d index=0x%X uid=0x%08X f2=0x%08X f3=0x%08X f4=0x%08X",rec,u32(a),u32(a+4),u32(a+8),u32(a+12),u32(a+16)));
  }
  dec(0x00436e30L);
  dec(0x00436f00L);
  d.dispose();
 }
}