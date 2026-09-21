import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class PursueNavRefreshEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw); disassemble(a); Function f=getFunctionAt(a);
  if(f==null) f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
 }
 @Override public void run() throws Exception {
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  dc(0x0046DFB0L,"tmp_PursueNav_Refresh",d);
  dc(0x0046E0E0L,"tmp_PursueNav_Enter",d);
  dc(0x0046E120L,"tmp_PursueNav_Event",d);
  d.dispose();
 }
}