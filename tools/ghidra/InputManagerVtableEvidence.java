import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class InputManagerVtableEvidence extends GhidraScript {
  DecompInterface d=new DecompInterface();
  long u32(long a)throws Exception{
    Memory m=currentProgram.getMemory(); byte[] b=new byte[4]; m.getBytes(toAddr(a),b);
    return ((long)b[0]&255)|(((long)b[1]&255)<<8)|(((long)b[2]&255)<<16)|(((long)b[3]&255)<<24);
  }
  void dec(long x)throws Exception{
    Function f=getFunctionAt(toAddr(x));
    if(f==null) f=getFunctionContaining(toAddr(x));
    println(String.format("\n=== TARGET 0x%08X ===",x));
    if(f==null){println("NO_FUNCTION");return;}
    println(f.getName()+" @ "+f.getEntryPoint());
    DecompileResults r=d.decompileFunction(f,60,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
  }
  public void run()throws Exception{
    d.openProgram(currentProgram);
    long base=0x005f3ba0L;
    for(int off=0;off<=0xe0;off+=4){
      long p=u32(base+off);
      Function f=getFunctionAt(toAddr(p));
      println(String.format("+0x%02X 0x%08X %s",off,p,f==null?"":f.getName()));
    }
    long target=u32(base+0xc8);
    dec(target);
    d.dispose();
  }
}