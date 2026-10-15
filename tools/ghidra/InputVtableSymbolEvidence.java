import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.symbol.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.listing.Function;
public class InputVtableSymbolEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{
  Function f=getFunctionContaining(toAddr(x)); println(String.format("\nDEC 0x%08X",x));
  if(f==null){println("NO_FUNCTION");return;}
  println(f.getName()+" @ "+f.getEntryPoint());
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  SymbolIterator it=currentProgram.getSymbolTable().getSymbols("PTR_FUN_005f3ba0");
  Memory m=currentProgram.getMemory();
  while(it.hasNext()){
   Symbol s=it.next(); long base=s.getAddress().getOffset();
   println("SYMBOL "+s.getName()+" @ "+s.getAddress());
   for(int off=0;off<=0xd0;off+=4){
    long p=Integer.toUnsignedLong(m.getInt(toAddr(base+off)));
    println(String.format("+0x%02X -> 0x%08X",off,p));
    if(off==0xc8)dec(p);
   }
  }
  d.dispose();
 }
}