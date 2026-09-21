import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class PursueNavLocalFunctionsEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionContaining(a);
  if(f==null){f=createFunction(a,name);}
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()+"@"+f.getEntryPoint()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  long[] xs={0x0046DA20L,0x0046DAF0L,0x0046DB20L,0x0046DBB0L,0x0046DC30L,0x0046DF70L};
  for(long x:xs)dc(x,"tmp_"+Long.toHexString(x),d);
  d.dispose();
 }
}