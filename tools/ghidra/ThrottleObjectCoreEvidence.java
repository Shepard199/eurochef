import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class ThrottleObjectCoreEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);if(f==null)f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  long[] xs={0x00455FB0L,0x00456020L,0x004560C0L,0x00456140L,0x004561F0L,0x00456250L,0x004562D0L,0x00456400L,0x00456480L,0x00456500L,0x00456570L};
  for(long x:xs)dc(x,"tmp_"+Long.toHexString(x),d);
  d.dispose();
 }
}