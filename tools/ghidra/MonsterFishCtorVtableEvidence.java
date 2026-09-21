import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class MonsterFishCtorVtableEvidence extends GhidraScript {
 private long ptr(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
 private void dc(long a, DecompInterface d) throws Exception {
  Function f=getFunctionContaining(toAddr(a));
  println(String.format("\n=== 0x%08X %s ===",a,f==null?"<missing>":f.getName()));
  if(f==null)return;
  DecompileResults r=d.decompileFunction(f,60,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
 }
 @Override public void run() throws Exception {
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  dc(0x0047DF70L,d);
  d.dispose();
 }
}