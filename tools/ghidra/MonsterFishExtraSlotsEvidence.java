import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class MonsterFishExtraSlotsEvidence extends GhidraScript {
 private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
 private void rec(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);if(f==null)f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,60,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  long vt=0x005E84F0L;int[] slots={0xF8,0x10C,0x110};
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  for(int s:slots){long p=ptr(vt+s);println(String.format("slot+0x%03X -> 0x%08X",s,p));rec(p,"tmp_Fish_"+Integer.toHexString(s),d);}
  d.dispose();
 }
}