import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.util.*;
public class CurrentAttackerEvidence extends GhidraScript {
 private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);if(f==null)f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  dc(0x004563C0L,"tmp_CurrentAttacker_Select",d);
  long[][] rows={
   {0x005E2920L,0},{0x005E68D8L,1},{0x005E5AC8L,2},{0x005E43E0L,3},
   {0x005E4108L,4},{0x005E6A40L,5},{0x005E6BA8L,6}
  };
  Set<Long> uniq=new LinkedHashSet<>();
  for(long[] row:rows){long p=ptr(row[0]+0x15c);println(String.format("VT 0x%08X +15C -> 0x%08X",row[0],p));uniq.add(p);}
  for(long p:uniq)dc(p,"tmp_slot15c_"+Long.toHexString(p),d);
  d.dispose();
 }
}