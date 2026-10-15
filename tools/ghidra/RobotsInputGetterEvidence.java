import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.listing.Function;
public class RobotsInputGetterEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{
  Function f=getFunctionContaining(toAddr(x)); println(String.format("\n===0x%08X===",x));
  if(f==null){println("NO_FUNCTION");return;}
  println(f.getName()+" @ "+f.getEntryPoint());
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  Memory m=currentProgram.getMemory();
  long base=0x005f0ae0L;
  for(int off=0;off<=0xf0;off+=4){
   long p=Integer.toUnsignedLong(m.getInt(toAddr(base+off)));
   if(off>=0x90) println(String.format("+0x%02X -> 0x%08X",off,p));
   if(off==0xc8||off==0xe4||off==0x94)dec(p);
  }
  d.dispose();
 }
}