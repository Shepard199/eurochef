import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class GenericAttackGateDeepEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionContaining(a);if(f==null){f=getFunctionAt(a);if(f==null)f=createFunction(a,name);}
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  long[] a={0x0044F3C0L,0x0044F6A0L,0x0044EEE0L,0x00456EB0L,0x00456EC0L,0x00456F70L,0x00456F90L};
  int i=0;for(long x:a)dc(x,"tmp_ga_"+(i++),d);d.dispose();
 }
}