import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class CurrentAttackerWatchBotGateEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionContaining(a);if(f==null){f=getFunctionAt(a);if(f==null)f=createFunction(a,name);}
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  int v=getInt(toAddr(0x005E233CL));
  println(String.format("DAT_005E233C bits=0x%08X float=%.9f",Integer.toUnsignedLong(v),Float.intBitsToFloat(v)));
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  dc(0x0041CDF0L,"tmp_41CDF0",d);
  d.dispose();
 }
}